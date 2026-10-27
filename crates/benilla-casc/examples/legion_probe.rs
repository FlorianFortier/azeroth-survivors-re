//! A probe of a Legion install: opens it and writes the files asked for, by file data id or path,
//! to `<cache>/out/`.
//!
//! ```text
//! LEGION_URL=https://…/WoW_Legion_7.3.5.26972 LEGION_CACHE=/tmp/legion \
//!   cargo run -p benilla-casc --features remote --example legion_probe -- <id | path>…
//! ```

use std::path::PathBuf;

use benilla_casc::remote::Remote;
use benilla_casc::{Casc, Error, Result};

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
