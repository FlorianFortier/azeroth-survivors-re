//! The byte codec every message rides on: little-endian fixed-width numbers, `u16`-length-prefixed
//! strings and lists. A decode never panics and never allocates more than the frame it reads
//! could hold: a short or malformed frame is an [`WireError`], which the server answers by dropping
//! the frame and the game by ignoring it.

use std::fmt;

/// A frame could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// The frame ended before the message did.
    Truncated,
    /// A tag byte named no variant.
    BadTag(u8),
    /// A string was not UTF-8.
    BadUtf8,
    /// Bytes were left over after the message.
    Trailing,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => write!(f, "frame truncated"),
            WireError::BadTag(t) => write!(f, "unknown tag {t}"),
            WireError::BadUtf8 => write!(f, "string is not UTF-8"),
            WireError::Trailing => write!(f, "trailing bytes after message"),
        }
    }
}

impl std::error::Error for WireError {}

/// Appends values to a frame.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn vec3(&mut self, v: [f32; 3]) -> &mut Self {
        self.f32(v[0]).f32(v[1]).f32(v[2])
    }
    /// A string, truncated to 65535 bytes on a char boundary.
    pub fn str(&mut self, s: &str) -> &mut Self {
        let mut end = s.len().min(u16::MAX as usize);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.u16(end as u16);
        self.buf.extend_from_slice(&s.as_bytes()[..end]);
        self
    }
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32);
        self.buf.extend_from_slice(b);
        self
    }
    /// A list length; lists are capped at 65535 items.
    pub fn len(&mut self, n: usize) -> &mut Self {
        self.u16(n.min(u16::MAX as usize) as u16)
    }
    pub fn opt<T>(&mut self, v: Option<T>, mut put: impl FnMut(&mut Self, T)) -> &mut Self {
        match v {
            None => {
                self.u8(0);
            }
            Some(x) => {
                self.u8(1);
                put(self, x);
            }
        }
        self
    }
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// Reads values off a frame.
pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.at.checked_add(n).ok_or(WireError::Truncated)?;
        let s = self.buf.get(self.at..end).ok_or(WireError::Truncated)?;
        self.at = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, WireError> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16, WireError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Result<u32, WireError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Result<u64, WireError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn vec3(&mut self) -> Result<[f32; 3], WireError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn str(&mut self) -> Result<String, WireError> {
        let n = self.u16()? as usize;
        let b = self.take(n)?;
        String::from_utf8(b.to_vec()).map_err(|_| WireError::BadUtf8)
    }
    pub fn bytes(&mut self) -> Result<Vec<u8>, WireError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    /// A list length, checked against what is left: every item takes at least `min_item` bytes,
    /// so a forged length cannot make the decoder reserve more than the frame holds.
    pub fn len(&mut self, min_item: usize) -> Result<usize, WireError> {
        let n = self.u16()? as usize;
        if n.saturating_mul(min_item.max(1)) > self.buf.len() - self.at {
            return Err(WireError::Truncated);
        }
        Ok(n)
    }
    pub fn opt<T>(
        &mut self,
        get: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<Option<T>, WireError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(get(self)?)),
            t => Err(WireError::BadTag(t)),
        }
    }
    /// The message is over: anything left is an error.
    pub fn end(&self) -> Result<(), WireError> {
        if self.at == self.buf.len() {
            Ok(())
        } else {
            Err(WireError::Trailing)
        }
    }
}

/// A message that can be framed.
pub trait Wire: Sized {
    fn put(&self, w: &mut Writer);
    fn get(r: &mut Reader) -> Result<Self, WireError>;

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.put(&mut w);
        w.finish()
    }
    fn decode(buf: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(buf);
        let v = Self::get(&mut r)?;
        r.end()?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_strings_round_trip() {
        let mut w = Writer::new();
        w.u8(7)
            .bool(true)
            .u16(65000)
            .u32(0xDEAD_BEEF)
            .u64(u64::MAX - 3)
            .f32(-1.5)
            .str("Thrall à Orgrimmar")
            .bytes(&[1, 2, 3])
            .opt(Some(9u32), |w, v| {
                w.u32(v);
            })
            .opt(None::<u32>, |w, v| {
                w.u32(v);
            });
        let buf = w.finish();
        let mut r = Reader::new(&buf);
        assert_eq!(r.u8().unwrap(), 7);
        assert!(r.bool().unwrap());
        assert_eq!(r.u16().unwrap(), 65000);
        assert_eq!(r.u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.u64().unwrap(), u64::MAX - 3);
        assert_eq!(r.f32().unwrap(), -1.5);
        assert_eq!(r.str().unwrap(), "Thrall à Orgrimmar");
        assert_eq!(r.bytes().unwrap(), vec![1, 2, 3]);
        assert_eq!(r.opt(|r| r.u32()).unwrap(), Some(9));
        assert_eq!(r.opt(|r| r.u32()).unwrap(), None);
        r.end().unwrap();
    }

    #[test]
    fn short_frames_are_errors_not_panics() {
        let mut r = Reader::new(&[1, 2]);
        assert_eq!(r.u32(), Err(WireError::Truncated));
        let mut r = Reader::new(&[0xFF, 0xFF]);
        assert_eq!(r.str(), Err(WireError::Truncated));
    }

    #[test]
    fn a_forged_list_length_is_refused_before_allocating() {
        let mut r = Reader::new(&[0xFF, 0xFF, 0]);
        assert_eq!(r.len(4), Err(WireError::Truncated));
    }

    #[test]
    fn strings_truncate_on_a_char_boundary() {
        let long = "é".repeat(40_000);
        let mut w = Writer::new();
        w.str(&long);
        let buf = w.finish();
        let s = Reader::new(&buf).str().unwrap();
        assert!(s.len() <= u16::MAX as usize);
        assert!(s.chars().all(|c| c == 'é'));
    }
}
