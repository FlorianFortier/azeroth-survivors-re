//! A read-only CASC reader for a local World of Warcraft Legion install (7.3.5, build 26972): the
//! storage every client since Warlords of Draenor keeps its files in, in place of 1.12.1's MPQs.
//! It is a spike toward reading a later client's data beside the 1.12.1 base
//! (`docs/EXPANSIONS.md`): native only, and no part of the client reads it yet.
//!
//! A file is found in four steps, each a table of the install:
//! - `.build.info` names the active build's config (`Data/config/xx/yy/<key>`), which names the
//!   **encoding** and **root** files ([`config`]);
//! - the **root** maps a file data id (or the Jenkins hash of its path, [`jenkins`]) to the
//!   file's content key ([`root`]);
//! - the **encoding** file maps a content key to the encoded keys stored ([`encoding`]);
//! - the local **indices** (`Data/data/*.idx`) map an encoded key's first nine bytes to an
//!   archive, an offset and a size ([`idx`]), where the file lies BLTE-encoded ([`blte`]).
//!
//! The install is read through a [`Source`], so a test or a probe can serve it from anywhere
//! (`examples/legion_probe.rs` reads byte ranges of a remote copy).

pub mod blte;
pub mod config;
pub mod encoding;
pub mod idx;
pub mod jenkins;
pub mod root;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Why a read failed.
#[derive(Debug)]
pub enum Error {
    Io(String),
    /// A table did not read as its format says.
    Format(String),
    /// No such file in the tables.
    NotFound(String),
    /// A BLTE chunk is encrypted, and no key is known for it.
    Encrypted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io: {e}"),
            Error::Format(e) => write!(f, "format: {e}"),
            Error::NotFound(e) => write!(f, "not found: {e}"),
            Error::Encrypted => write!(f, "encrypted"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad(what: impl Into<String>) -> Error {
    Error::Format(what.into())
}

/// Where the install's files are read from: `path` is relative to the install's root
/// (`.build.info`, `Data/data/data.000`).
pub trait Source {
    /// The whole file.
    fn read_all(&self, path: &str) -> Result<Vec<u8>>;
    /// `len` bytes from `offset`.
    fn read_at(&self, path: &str, offset: u64, len: u64) -> Result<Vec<u8>>;
    /// The file names in a folder.
    fn list(&self, dir: &str) -> Result<Vec<String>>;
}

/// An install on disk.
pub struct Dir(pub PathBuf);

impl Source for Dir {
    fn read_all(&self, path: &str) -> Result<Vec<u8>> {
        std::fs::read(self.0.join(path)).map_err(|e| Error::Io(format!("{path}: {e}")))
    }

    fn read_at(&self, path: &str, offset: u64, len: u64) -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(self.0.join(path))
            .map_err(|e| Error::Io(format!("{path}: {e}")))?;
        f.seek(SeekFrom::Start(offset))
            .map_err(|e| Error::Io(e.to_string()))?;
        let mut out = vec![0; len as usize];
        f.read_exact(&mut out)
            .map_err(|e| Error::Io(format!("{path} @{offset}+{len}: {e}")))?;
        Ok(out)
    }

    fn list(&self, dir: &str) -> Result<Vec<String>> {
        let rd =
            std::fs::read_dir(self.0.join(dir)).map_err(|e| Error::Io(format!("{dir}: {e}")))?;
        Ok(rd
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .collect())
    }
}

/// An open install: its indices, encoding and root, ready to read files by id or by path.
pub struct Casc<S: Source> {
    source: S,
    index: HashMap<[u8; 9], idx::Entry>,
    encoding: encoding::Encoding,
    root: root::Root,
    /// The locale files are taken in, as root's locale flags (enUS, 0x2).
    pub locale: u32,
}

impl<S: Source> Casc<S> {
    /// Open the active build named by `.build.info`.
    pub fn open(source: S) -> Result<Self> {
        let info = String::from_utf8_lossy(&source.read_all(".build.info")?).into_owned();
        let build_key = config::active_build_key(&info)?;
        let build = config::parse(&source.read_all(&config::config_path(&build_key))?);
        let index = idx::load(&source)?;
        let enc_key = build
            .get("encoding")
            .and_then(|v| v.get(1))
            .ok_or_else(|| bad("build config has no encoding key"))?;
        let enc_blte = read_ekey(&source, &index, &hex(enc_key)?)?;
        let encoding = encoding::Encoding::parse(&blte::decode(&enc_blte)?)?;
        let root_ckey = build
            .get("root")
            .and_then(|v| v.first())
            .ok_or_else(|| bad("build config has no root key"))?;
        let root_ekey = encoding
            .ekey(&hex(root_ckey)?)
            .ok_or_else(|| Error::NotFound("root in encoding".into()))?;
        let root = root::Root::parse(&blte::decode(&read_ekey(&source, &index, &root_ekey)?)?)?;
        Ok(Self {
            source,
            index,
            encoding,
            root,
            locale: root::LOCALE_EN_US,
        })
    }

    /// The root's file data ids, how many.
    pub fn file_count(&self) -> usize {
        self.root.len()
    }

    /// A file by its file data id.
    pub fn read_id(&self, file_data_id: u32) -> Result<Vec<u8>> {
        let ckey = self
            .root
            .by_id(file_data_id, self.locale)
            .ok_or_else(|| Error::NotFound(format!("file data id {file_data_id}")))?;
        self.read_ckey(&ckey)
    }

    /// A file by its path (`creature/felbat/felbat.m2`), through the root's name hashes.
    pub fn read_path(&self, path: &str) -> Result<Vec<u8>> {
        let id = self
            .id_of(path)
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        self.read_id(id)
    }

    /// A path's file data id, if the root names it.
    pub fn id_of(&self, path: &str) -> Option<u32> {
        self.root.by_hash(jenkins::path_hash(path))
    }

    /// A file by its content key.
    pub fn read_ckey(&self, ckey: &[u8; 16]) -> Result<Vec<u8>> {
        let ekey = self
            .encoding
            .ekey(ckey)
            .ok_or_else(|| Error::NotFound("content key in encoding".into()))?;
        blte::decode(&read_ekey(&self.source, &self.index, &ekey)?)
    }
}

/// The BLTE bytes stored under an encoded key: the local index's archive, offset and size, less
/// the 30-byte header each stored file carries.
fn read_ekey<S: Source>(
    source: &S,
    index: &HashMap<[u8; 9], idx::Entry>,
    ekey: &[u8; 16],
) -> Result<Vec<u8>> {
    let mut short = [0u8; 9];
    short.copy_from_slice(&ekey[..9]);
    let e = index
        .get(&short)
        .ok_or_else(|| Error::NotFound(format!("encoded key {} in the indices", to_hex(ekey))))?;
    let path = format!("Data/data/data.{:03}", e.archive);
    let raw = source.read_at(&path, e.offset, u64::from(e.size))?;
    if raw.len() < idx::STORED_HEADER {
        return Err(bad("stored file shorter than its header"));
    }
    Ok(raw[idx::STORED_HEADER..].to_vec())
}

/// A hex string's 16 bytes.
pub fn hex(s: &str) -> Result<[u8; 16]> {
    let s = s.trim();
    if s.len() != 32 {
        return Err(bad(format!("not a 16-byte key: {s}")));
    }
    let mut out = [0u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| bad(format!("hex {s}")))?;
    }
    Ok(out)
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
