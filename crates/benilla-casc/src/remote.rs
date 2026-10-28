//! A remote copy of an install read by byte ranges with `curl`, each kept in a cache folder, so a
//! 74 GB install is read a few megabytes at a time: the probes' and tests' source, and the
//! client's when a player streams Legion's data (`scripts/fetch-legion.sh --stream`).
//!
//! A folder listing comes from `<cache>/<folder>.list` (one name a line), which the caller writes
//! from the remote copy's file list (the fetch script does). Nothing it fetches belongs in git.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result, Source};

pub struct Remote {
    pub url: String,
    pub cache: PathBuf,
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
