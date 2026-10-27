//! WoWDBDefs' table definitions (`.dbd`): a table's columns and, per build, its layout. They are
//! WoWDBDefs' data (CC BY-SA 4.0), read from a local folder at run time and never copied here.
//!
//! A file holds `COLUMNS` (one a line: a type, `int`, `float`, `string` or `locstring`, an
//! optional `<Table::Field>` foreign key, the name, `?` when unverified), then blocks, a blank
//! line between them: `LAYOUT` lines (the layout hashes a DB2 header carries), `BUILD` lines,
//! `COMMENT`, and the fields in file order: `$annotations$` (`id`, `noninline`, `relation`), the
//! name, `<32>` or `<u16>` for an int's size and sign, `[3]` for an array, `// a comment`.

use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Float,
    String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub kind: Kind,
    /// An int's width in bits; 32 for a float or a string.
    pub bits: u32,
    pub signed: bool,
    /// The element count of an array, 1 for a plain field.
    pub array: u32,
    /// The record's id.
    pub id: bool,
    /// Not in the record: the id list's or the relationship map's.
    pub noninline: bool,
    /// The relationship map's foreign key.
    pub relation: bool,
}

/// One layout of a table: its hashes, its builds, its fields.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub hashes: Vec<u32>,
    pub builds: Vec<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Default)]
pub struct Definition {
    pub layouts: Vec<Layout>,
}

impl Definition {
    pub fn parse(text: &str) -> Result<Self> {
        let mut columns: Vec<(String, Kind)> = Vec::new();
        let mut layouts = Vec::new();
        let mut in_columns = false;
        let mut current: Option<Layout> = None;
        for raw in text.lines() {
            let line = raw.split("//").next().unwrap_or("").trim();
            if line.is_empty() {
                in_columns = false;
                if let Some(l) = current.take() {
                    layouts.push(l);
                }
                continue;
            }
            if line == "COLUMNS" {
                in_columns = true;
                continue;
            }
            if in_columns {
                let mut words = line.split_whitespace();
                let (Some(ty), Some(name)) = (words.next(), words.next()) else {
                    continue;
                };
                let kind = match ty.split('<').next().unwrap_or(ty) {
                    "float" => Kind::Float,
                    "string" | "locstring" => Kind::String,
                    _ => Kind::Int,
                };
                columns.push((name.trim_end_matches('?').to_string(), kind));
                continue;
            }
            let layout = current.get_or_insert_with(Layout::default);
            if let Some(rest) = line.strip_prefix("LAYOUT ") {
                layout.hashes.extend(
                    rest.split(',')
                        .filter_map(|h| u32::from_str_radix(h.trim(), 16).ok()),
                );
            } else if let Some(rest) = line.strip_prefix("BUILD ") {
                layout
                    .builds
                    .extend(rest.split(',').map(|b| b.trim().to_string()));
            } else if line.starts_with("COMMENT") {
            } else {
                layout.fields.push(field(line, &columns)?);
            }
        }
        if let Some(l) = current.take() {
            layouts.push(l);
        }
        Ok(Self { layouts })
    }

    /// The layout a DB2 header's hash names, else the one listing `build`.
    pub fn layout(&self, hash: u32, build: &str) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|l| l.hashes.contains(&hash))
            .or_else(|| {
                self.layouts
                    .iter()
                    .find(|l| l.builds.iter().any(|b| build_in(build, b)))
            })
    }
}

fn field(line: &str, columns: &[(String, Kind)]) -> Result<Field> {
    let (notes, rest) = match line.strip_prefix('$') {
        Some(r) => r
            .split_once('$')
            .ok_or_else(|| Error::Format(format!("dbd field {line}")))?,
        None => ("", line),
    };
    let (rest, array) = match rest.split_once('[') {
        Some((r, a)) => (r, a.trim_end_matches(']').parse().unwrap_or(1)),
        None => (rest, 1),
    };
    let (name, size) = match rest.split_once('<') {
        Some((n, s)) => (n, Some(s.trim_end_matches('>'))),
        None => (rest, None),
    };
    let kind = columns
        .iter()
        .find(|(n, _)| n == name)
        .map_or(Kind::Int, |(_, k)| *k);
    let (signed, bits) = match size {
        Some(s) => match s.strip_prefix('u') {
            Some(b) => (false, b.parse().unwrap_or(32)),
            None => (true, s.parse().unwrap_or(32)),
        },
        None => (kind == Kind::Int, 32),
    };
    let notes: Vec<&str> = notes.split(',').map(str::trim).collect();
    Ok(Field {
        name: name.to_string(),
        kind,
        bits,
        signed,
        array,
        id: notes.contains(&"id"),
        noninline: notes.contains(&"noninline"),
        relation: notes.contains(&"relation"),
    })
}

/// Whether `build` (`7.3.5.26972`) is `entry`, or within `entry`'s range (`a-b`).
fn build_in(build: &str, entry: &str) -> bool {
    let key = |b: &str| -> Vec<u32> { b.split('.').filter_map(|p| p.parse().ok()).collect() };
    match entry.split_once('-') {
        Some((lo, hi)) => key(lo) <= key(build) && key(build) <= key(hi),
        None => entry == build,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: &str = "COLUMNS\nint ID\nint<Other::ID> ModelID\nfloat Scale\nstring Name?\nint Tex\n\n\
        LAYOUT 0000ABCD\nBUILD 7.3.0.1-7.3.5.26972\n$noninline,id$ID<32>\nScale\nModelID<u16>\nName\nTex<32>[3]\n\
        $noninline,relation$ModelID<32>\n\nLAYOUT 1234\nBUILD 8.0.1.1\n$id$ID<32>\n";

    #[test]
    fn a_layout_is_found_by_its_hash_or_its_build() {
        let d = Definition::parse(DEF).unwrap();
        let l = d.layout(0xabcd, "").unwrap();
        let names: Vec<&str> = l.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["ID", "Scale", "ModelID", "Name", "Tex", "ModelID"]);
        assert!(l.fields[0].id && l.fields[0].noninline);
        assert_eq!((l.fields[1].kind, l.fields[1].bits), (Kind::Float, 32));
        assert_eq!((l.fields[2].bits, l.fields[2].signed), (16, false));
        assert_eq!(l.fields[3].kind, Kind::String);
        assert_eq!(l.fields[4].array, 3);
        assert!(l.fields[5].relation);
        assert!(std::ptr::eq(d.layout(0, "7.3.5.26972").unwrap(), l));
        assert!(d.layout(0, "7.3.5.26973").is_none());
    }
}
