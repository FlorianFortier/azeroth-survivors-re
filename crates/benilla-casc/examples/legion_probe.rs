//! A probe of a Legion install kept remote: every read is a byte range fetched with `curl` and
//! kept in a cache folder, so a 74 GB install is read a few megabytes at a time.
//!
//! ```text
//! LEGION_URL=https://…/WoW_Legion_7.3.5.26972 LEGION_CACHE=/tmp/legion \
//!   cargo run -p benilla-casc --example legion_probe -- <file data id | path>…
//! ```
//!
//! A folder listing is read from `<cache>/<folder>.list` (one name a line), which the caller
//! writes from the remote copy's file list. Each file asked for is written to `<cache>/out/`.
//! Nothing it fetches belongs in git.

use std::path::{Path, PathBuf};
use std::process::Command;

use benilla_casc::{Casc, Error, Result, Source};

struct Remote {
    url: String,
    cache: PathBuf,
}

impl Remote {
    fn fetch(&self, path: &str, range: Option<(u64, u64)>, into: &Path) -> Result<()> {
        if let Some(dir) = into.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::Io(e.to_string()))?;
        }
        let mut cmd = Command::new("curl");
        cmd.args(["-sS", "-f", "-L", "--retry", "4", "-o"])
            .arg(into)
            .arg(format!("{}/{}", self.url, path));
        if let Some((at, len)) = range {
            cmd.arg("-r").arg(format!("{at}-{}", at + len - 1));
        }
        let status = cmd.status().map_err(|e| Error::Io(e.to_string()))?;
        if !status.success() {
            let _ = std::fs::remove_file(into);
            return Err(Error::Io(format!("curl {path} {range:?}: {status}")));
        }
        Ok(())
    }
}

impl Source for Remote {
    fn read_all(&self, path: &str) -> Result<Vec<u8>> {
        let local = self.cache.join(path);
        if !local.exists() {
            self.fetch(path, None, &local)?;
        }
        std::fs::read(&local).map_err(|e| Error::Io(e.to_string()))
    }

    fn read_at(&self, path: &str, offset: u64, len: u64) -> Result<Vec<u8>> {
        let local = self
            .cache
            .join("ranges")
            .join(format!("{}@{offset}+{len}", path.replace('/', "_")));
        if !local.exists() {
            self.fetch(path, Some((offset, len)), &local)?;
        }
        let data = std::fs::read(&local).map_err(|e| Error::Io(e.to_string()))?;
        if data.len() as u64 != len {
            return Err(Error::Io(format!(
                "{path} @{offset}: {} bytes of {len}",
                data.len()
            )));
        }
        Ok(data)
    }

    fn list(&self, dir: &str) -> Result<Vec<String>> {
        let list = self.cache.join(format!("{dir}.list"));
        let text = std::fs::read_to_string(&list)
            .map_err(|e| Error::Io(format!("{}: {e}", list.display())))?;
        Ok(text.lines().map(|l| l.trim().to_string()).collect())
    }
}

fn main() -> Result<()> {
    let url = std::env::var("LEGION_URL").map_err(|_| Error::Io("LEGION_URL".into()))?;
    let cache =
        PathBuf::from(std::env::var("LEGION_CACHE").map_err(|_| Error::Io("LEGION_CACHE".into()))?);
    let casc = Casc::open(Remote {
        url,
        cache: cache.clone(),
    })?;
    println!("root: {} file data ids", casc.file_count());
    for arg in std::env::args().skip(1) {
        let (id, data) = match arg.parse::<u32>() {
            Ok(id) => (Some(id), casc.read_id(id)),
            Err(_) => (casc.id_of(&arg), casc.read_path(&arg)),
        };
        match data {
            Ok(bytes) => {
                let name = arg.replace(['/', '\\'], "_");
                let out = cache.join("out").join(&name);
                std::fs::create_dir_all(cache.join("out")).map_err(|e| Error::Io(e.to_string()))?;
                std::fs::write(&out, &bytes).map_err(|e| Error::Io(e.to_string()))?;
                let magic = String::from_utf8_lossy(&bytes[..bytes.len().min(4)]).into_owned();
                println!("{arg} (id {id:?}): {} bytes, {magic:?}", bytes.len());
            }
            Err(e) => println!("{arg} (id {id:?}): {e}"),
        }
    }
    Ok(())
}
