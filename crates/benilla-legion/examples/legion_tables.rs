//! Read Legion's DB2 tables from a remote Legion install, each typed by its WoWDBDefs definition,
//! and print each one's size and a row.
//!
//! ```text
//! LEGION_URL=… LEGION_CACHE=/tmp/legion LEGION_DBD=/tmp/WoWDBDefs/definitions \
//!   cargo run --release -p benilla-legion --example legion_tables -- CreatureDisplayInfo ItemDisplayInfo…
//! ```

use std::path::{Path, PathBuf};

use benilla_casc::remote::Remote;
use benilla_casc::Casc;
use benilla_legion::db2::{Raw, Table};
use benilla_legion::dbd::Definition;

type Res<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Res<()> {
    let casc = Casc::open(Remote {
        url: std::env::var("LEGION_URL")?,
        cache: PathBuf::from(std::env::var("LEGION_CACHE")?),
    })?;
    let dir = std::env::var("LEGION_DBD")?;
    for name in std::env::args().skip(1) {
        let raw = Raw::parse(&casc.read_path(&format!("DBFilesClient/{name}.db2"))?)?;
        let def = Definition::parse(&std::fs::read_to_string(
            Path::new(&dir).join(format!("{name}.dbd")),
        )?)?;
        let Some(layout) = def.layout(raw.layout_hash, "7.3.5.26972") else {
            println!("{name}: no layout {:08X}", raw.layout_hash);
            continue;
        };
        let t = Table::new(&raw, layout);
        let sample = t.rows.values().nth(t.rows.len() / 2);
        println!(
            "{name}: layout {:08X}, {} records, {} rows with copies; row {:?}",
            raw.layout_hash,
            raw.record_count,
            t.rows.len(),
            sample.map(|r| {
                let mut v: Vec<String> = layout
                    .fields
                    .iter()
                    .filter_map(|f| r.get(&f.name).map(|x| format!("{}={x:?}", f.name)))
                    .collect();
                v.dedup();
                v
            })
        );
    }
    Ok(())
}
