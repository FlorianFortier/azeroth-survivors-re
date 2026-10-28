//! Legion's DB2 tables, format `WDC1` (7.3.5): an 84-byte header, each field's size and position,
//! the records (fixed size, unless the table is sparse), the string table, then the id list, the
//! copy table, each field's storage, the pallet, the common data and the relationship map.
//!
//! A field is stored one of five ways ([`Storage`]): plainly in the record, bit-packed in it, as
//! the common data's value for the row's id (else a default), or as an index into the pallet (one
//! value, or one array). A table is read raw ([`Raw`], each field's `u32` words) and then typed by
//! a WoWDBDefs layout ([`Table`]), whose inline fields are the file's fields in order.
//!
//! Sparse tables (an offset map in place of fixed records) are not read: none the spike needs is.

use std::collections::{BTreeMap, HashMap};

use crate::dbd::{Field, Kind, Layout};
use crate::{u16_at, u32_at, Error, Result};

const HEADER: usize = 84;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Plain,
    Bitpacked,
    Common { default: u32 },
    Pallet,
    PalletArray { count: u32 },
}

#[derive(Clone, Copy, Debug)]
struct FieldInfo {
    offset_bits: u32,
    size_bits: u32,
    storage: Storage,
    /// Where the field's pallet or common data starts, in bytes.
    data_at: usize,
}

/// A DB2 table as stored: rows of `u32` words, before any layout names them.
pub struct Raw {
    pub layout_hash: u32,
    pub record_count: usize,
    record_size: usize,
    records: Vec<u8>,
    strings: Vec<u8>,
    ids: Vec<u32>,
    fields: Vec<FieldInfo>,
    pallet: Vec<u8>,
    common: Vec<HashMap<u32, u32>>,
    relations: HashMap<usize, u32>,
    pub copies: Vec<(u32, u32)>,
}

impl Raw {
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.get(0..4) != Some(b"WDC1") {
            return Err(Error::Format("not WDC1".into()));
        }
        let h = |i: usize| u32_at(b, i * 4).ok_or(Error::Truncated("db2 header"));
        let (record_count, field_count, record_size, string_size) = (
            h(1)? as usize,
            h(2)? as usize,
            h(3)? as usize,
            h(4)? as usize,
        );
        let layout_hash = h(6)?;
        let copy_size = h(10)? as usize;
        let flags = u16_at(b, 44).ok_or(Error::Truncated("db2 flags"))?;
        let id_index = u16_at(b, 46).ok_or(Error::Truncated("db2 id index"))? as usize;
        let id_list_size = u32_at(b, 64).ok_or(Error::Truncated("db2"))? as usize;
        let storage_size = u32_at(b, 68).ok_or(Error::Truncated("db2"))? as usize;
        let common_size = u32_at(b, 72).ok_or(Error::Truncated("db2"))? as usize;
        let pallet_size = u32_at(b, 76).ok_or(Error::Truncated("db2"))? as usize;
        let relation_size = u32_at(b, 80).ok_or(Error::Truncated("db2"))? as usize;
        if flags & 0x1 != 0 {
            return Err(Error::Format("a sparse DB2 (offset map)".into()));
        }
        let take = |at: &mut usize, n: usize, what: &'static str| -> Result<Vec<u8>> {
            let s = b.get(*at..*at + n).ok_or(Error::Truncated(what))?.to_vec();
            *at += n;
            Ok(s)
        };
        let mut at = HEADER + field_count * 4;
        let records = take(&mut at, record_count * record_size, "db2 records")?;
        let strings = take(&mut at, string_size, "db2 strings")?;
        let id_list = take(&mut at, id_list_size, "db2 id list")?;
        let copy = take(&mut at, copy_size, "db2 copy table")?;
        let storage = take(&mut at, storage_size, "db2 field storage")?;
        let pallet = take(&mut at, pallet_size, "db2 pallet")?;
        let common_data = take(&mut at, common_size, "db2 common data")?;
        let relation = take(&mut at, relation_size, "db2 relationships")?;

        let words = |v: &[u8]| -> Vec<u32> {
            v.as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect()
        };
        let (mut pallet_at, mut common_at) = (0usize, 0usize);
        let mut fields = Vec::new();
        let mut common = Vec::new();
        for s in storage.as_chunks::<24>().0 {
            let w = words(s);
            let offset_bits = u32::from(u16::from_le_bytes([s[0], s[1]]));
            let size_bits = u32::from(u16::from_le_bytes([s[2], s[3]]));
            let extra = w[1] as usize;
            let (storage, data_at) = match w[2] {
                0 => (Storage::Plain, 0),
                1 => (Storage::Bitpacked, 0),
                2 => {
                    let block = common_data
                        .get(common_at..common_at + extra)
                        .ok_or(Error::Truncated("db2 common block"))?;
                    let map = words(block)
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| (p[0], p[1]))
                        .collect();
                    common.push(map);
                    common_at += extra;
                    (Storage::Common { default: w[3] }, common.len() - 1)
                }
                3 => {
                    pallet_at += extra;
                    (Storage::Pallet, pallet_at - extra)
                }
                4 => {
                    pallet_at += extra;
                    (Storage::PalletArray { count: w[5] }, pallet_at - extra)
                }
                k => return Err(Error::Format(format!("db2 storage type {k}"))),
            };
            fields.push(FieldInfo {
                offset_bits,
                size_bits,
                storage,
                data_at,
            });
        }

        let mut raw = Self {
            layout_hash,
            record_count,
            record_size,
            records,
            strings,
            ids: words(&id_list),
            fields,
            pallet,
            common,
            relations: HashMap::new(),
            copies: words(&copy)
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| (p[0], p[1]))
                .collect(),
        };
        if raw.ids.is_empty() {
            raw.ids = (0..record_count)
                .map(|r| raw.words(r, id_index, 32).first().copied().unwrap_or(0))
                .collect();
        }
        if relation.len() >= 12 {
            let n = u32_at(&relation, 0).unwrap_or(0) as usize;
            for e in relation[12..].as_chunks::<8>().0.iter().take(n) {
                let foreign = u32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                let row = u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as usize;
                raw.relations.insert(row, foreign);
            }
        }
        Ok(raw)
    }

    pub fn id(&self, row: usize) -> u32 {
        self.ids.get(row).copied().unwrap_or(0)
    }

    pub fn relation(&self, row: usize) -> Option<u32> {
        self.relations.get(&row).copied()
    }

    /// `bits` wide words from the record's bits at `offset`.
    fn bits(&self, row: usize, offset: u32, size: u32) -> u64 {
        let rec = &self.records[row * self.record_size..(row + 1) * self.record_size];
        let first = (offset / 8) as usize;
        let mut buf = [0u8; 8];
        let end = (first + 8).min(rec.len());
        if first < end {
            buf[..end - first].copy_from_slice(&rec[first..end]);
        }
        let v = u64::from_le_bytes(buf) >> (offset % 8);
        if size >= 64 {
            v
        } else {
            v & ((1u64 << size) - 1)
        }
    }

    /// Field `field` of row `row` as `u32` words: one, or an array's (`elem_bits` each).
    pub fn words(&self, row: usize, field: usize, elem_bits: u32) -> Vec<u32> {
        let Some(f) = self.fields.get(field) else {
            return Vec::new();
        };
        match f.storage {
            Storage::Plain => {
                let n = (f.size_bits / elem_bits.max(1)).max(1);
                (0..n)
                    .map(|i| self.bits(row, f.offset_bits + i * elem_bits, elem_bits) as u32)
                    .collect()
            }
            Storage::Bitpacked => vec![self.bits(row, f.offset_bits, f.size_bits) as u32],
            Storage::Common { default } => {
                let id = self.id(row);
                vec![self.common[f.data_at].get(&id).copied().unwrap_or(default)]
            }
            Storage::Pallet => {
                let i = self.bits(row, f.offset_bits, f.size_bits) as usize;
                vec![u32_at(&self.pallet, f.data_at + i * 4).unwrap_or(0)]
            }
            Storage::PalletArray { count } => {
                let i = self.bits(row, f.offset_bits, f.size_bits) as usize;
                (0..count as usize)
                    .map(|j| {
                        u32_at(&self.pallet, f.data_at + (i * count as usize + j) * 4).unwrap_or(0)
                    })
                    .collect()
            }
        }
    }

    /// The string the string table holds at `offset`.
    pub fn string(&self, offset: u32) -> String {
        let s = self.strings.get(offset as usize..).unwrap_or(&[]);
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf8_lossy(&s[..end]).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f32),
    Str(String),
    Ints(Vec<i64>),
    Floats(Vec<f32>),
}

/// A row, its values by field name.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub id: u32,
    values: HashMap<String, Value>,
}

impl Row {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        match self.values.get(name)? {
            Value::Int(v) => Some(*v),
            Value::Ints(v) => v.first().copied(),
            _ => None,
        }
    }

    pub fn ints(&self, name: &str) -> Vec<i64> {
        match self.values.get(name) {
            Some(Value::Ints(v)) => v.clone(),
            Some(Value::Int(v)) => vec![*v],
            _ => Vec::new(),
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match self.values.get(name)? {
            Value::Str(v) => Some(v),
            _ => None,
        }
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        match self.values.get(name)? {
            Value::Float(v) => Some(*v),
            Value::Floats(v) => v.first().copied(),
            _ => None,
        }
    }
}

/// A table typed by a layout: its rows by id, copies included.
pub struct Table {
    pub rows: BTreeMap<u32, Row>,
}

impl Table {
    pub fn new(raw: &Raw, layout: &Layout) -> Self {
        let mut rows = BTreeMap::new();
        for r in 0..raw.record_count {
            let id = raw.id(r);
            let mut values = HashMap::new();
            let mut file_field = 0;
            for f in &layout.fields {
                if f.noninline {
                    if f.relation {
                        if let Some(v) = raw.relation(r) {
                            values.insert(f.name.clone(), Value::Int(i64::from(v)));
                        }
                    } else if f.id {
                        values.insert(f.name.clone(), Value::Int(i64::from(id)));
                    }
                    continue;
                }
                let words = raw.words(r, file_field, f.bits);
                file_field += 1;
                values.insert(f.name.clone(), typed(raw, f, &words));
            }
            rows.insert(id, Row { id, values });
        }
        for &(new, from) in &raw.copies {
            if let Some(row) = rows.get(&from).cloned() {
                let mut row = row;
                row.id = new;
                for f in layout.fields.iter().filter(|f| f.id) {
                    row.values
                        .insert(f.name.clone(), Value::Int(i64::from(new)));
                }
                rows.insert(new, row);
            }
        }
        Self { rows }
    }

    pub fn get(&self, id: u32) -> Option<&Row> {
        self.rows.get(&id)
    }
}

fn typed(raw: &Raw, f: &Field, words: &[u32]) -> Value {
    let int = |w: u32| -> i64 {
        if f.signed && f.bits < 64 {
            let shift = 64 - f.bits.min(32);
            ((i64::from(w) << shift) >> shift) as i32 as i64
        } else {
            i64::from(w)
        }
    };
    match (f.kind, f.array > 1) {
        (Kind::Float, false) => Value::Float(f32::from_bits(words.first().copied().unwrap_or(0))),
        (Kind::Float, true) => Value::Floats(words.iter().map(|&w| f32::from_bits(w)).collect()),
        (Kind::String, _) => Value::Str(raw.string(words.first().copied().unwrap_or(0))),
        (Kind::Int, false) => Value::Int(int(words.first().copied().unwrap_or(0))),
        (Kind::Int, true) => Value::Ints(words.iter().map(|&w| int(w)).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbd::Definition;

    /// A two-row WDC1: a plain u16, a bit-packed field, a common one, a pallet one, an id list.
    fn table() -> Vec<u8> {
        let (rows, fields, rsize) = (2u32, 4u32, 4u32);
        let mut b = b"WDC1".to_vec();
        let header = [
            rows, fields, rsize, 1, 0, 0xABCD, 10, 11, 0, 0, // .. copy size
        ];
        for v in header {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0x14u16.to_le_bytes()); // flags: an id list
        b.extend_from_slice(&0u16.to_le_bytes()); // id index
        let storage = 4 * 24;
        for v in [
            fields, 0, 0, 0, 8, storage, 8, /* common */
            8, /* pallet */
            0,
        ] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for _ in 0..fields {
            b.extend_from_slice(&[0; 4]);
        }
        // Records: u16 @0, 4 bits @16, the pallet index 4 bits @20.
        b.extend_from_slice(&[7, 0, 0x13, 0]); // 7, packed 3, pallet index 1
        b.extend_from_slice(&[9, 0, 0x05, 0]); // 9, packed 5, pallet index 0
        b.push(0); // strings
        for id in [10u32, 11] {
            b.extend_from_slice(&id.to_le_bytes());
        }
        let info = |off: u16, size: u16, extra: u32, kind: u32, v1: u32| {
            let mut s = Vec::new();
            s.extend_from_slice(&off.to_le_bytes());
            s.extend_from_slice(&size.to_le_bytes());
            for v in [extra, kind, v1, 0, 0] {
                s.extend_from_slice(&v.to_le_bytes());
            }
            s
        };
        b.extend(info(0, 16, 0, 0, 0));
        b.extend(info(16, 4, 0, 1, 0));
        b.extend(info(0, 0, 8, 2, 42));
        b.extend(info(20, 4, 8, 3, 0));
        for v in [100u32, 200] {
            b.extend_from_slice(&v.to_le_bytes()); // pallet
        }
        for v in [11u32, 99] {
            b.extend_from_slice(&v.to_le_bytes()); // common: id 11 → 99
        }
        b
    }

    #[test]
    fn every_storage_reads_its_value() {
        let raw = Raw::parse(&table()).unwrap();
        let def = Definition::parse(
            "COLUMNS\nint ID\nint A\nint B\nint C\nint D\n\nLAYOUT 0000ABCD\n\
             $noninline,id$ID<32>\nA<u16>\nB<u8>\nC<32>\nD<32>\n",
        )
        .unwrap();
        let t = Table::new(&raw, def.layout(0xABCD, "").unwrap());
        let (a, b) = (t.get(10).unwrap(), t.get(11).unwrap());
        assert_eq!([a.int("A"), b.int("A")], [Some(7), Some(9)]);
        assert_eq!([a.int("B"), b.int("B")], [Some(3), Some(5)]);
        assert_eq!([a.int("C"), b.int("C")], [Some(42), Some(99)]);
        assert_eq!([a.int("D"), b.int("D")], [Some(200), Some(100)]);
        assert_eq!(a.int("ID"), Some(10));
    }
}
