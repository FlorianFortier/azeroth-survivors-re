//! The install's text tables: `.build.info` (a `|`-separated table, one row a region, `Active`
//! the one in use) and a build config (`key = value value…` lines), at
//! `Data/config/<k[0..2]>/<k[2..4]>/<k>`.

use std::collections::HashMap;

use crate::{bad, Result};

/// The active row's `Build Key`.
pub fn active_build_key(build_info: &str) -> Result<String> {
    let mut lines = build_info.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| bad(".build.info is empty"))?
        .split('|')
        .map(|h| h.split('!').next().unwrap_or(h))
        .collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (key, active) = (
        col("Build Key").ok_or_else(|| bad(".build.info has no Build Key"))?,
        col("Active"),
    );
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split('|').collect()).collect();
    let row = rows
        .iter()
        .find(|r| active.is_some_and(|a| r.get(a) == Some(&"1")))
        .or(rows.first())
        .ok_or_else(|| bad(".build.info has no row"))?;
    row.get(key)
        .map(|k| k.to_string())
        .ok_or_else(|| bad(".build.info row too short"))
}

/// Where a config with key `key` lies.
pub fn config_path(key: &str) -> String {
    format!("Data/config/{}/{}/{key}", &key[0..2], &key[2..4])
}

/// A config's `key = values` lines; comments (`#`) skipped.
pub fn parse(text: &[u8]) -> HashMap<String, Vec<String>> {
    String::from_utf8_lossy(text)
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((
                k.trim().to_string(),
                v.split_whitespace().map(str::to_string).collect(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_active_row_names_the_build() {
        let info = "Branch!STRING:0|Active!DEC:1|Build Key!HEX:16\neu|0|aa\nus|1|bb\n";
        assert_eq!(active_build_key(info).unwrap(), "bb");
        assert_eq!(config_path("3b0517b5"), "Data/config/3b/05/3b0517b5");
        let c = parse(b"# comment\nroot = 0011\nencoding = aa bb\n");
        assert_eq!(c["encoding"], ["aa", "bb"]);
        assert_eq!(c["root"], ["0011"]);
    }
}
