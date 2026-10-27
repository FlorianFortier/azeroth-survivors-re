//! BLTE, the encoding every stored file is in: `BLTE`, a big-endian header size, then (when that
//! size is not 0) a chunk table, `0x0F` and a 24-bit count, each chunk's encoded and decoded size
//! and checksum; each chunk a mode byte and its body: `N` as is, `Z` zlib, `E` encrypted (refused:
//! no keys here), `F` a nested BLTE.

use std::io::Read;

use crate::{bad, Error, Result};

pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    if data.get(0..4) != Some(b"BLTE") {
        return Err(bad("not BLTE"));
    }
    let be = |at: usize| -> Result<u32> {
        data.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| bad("BLTE truncated"))
    };
    let header = be(4)? as usize;
    if header == 0 {
        return chunk(&data[8..]);
    }
    let count = (be(8)? & 0x00ff_ffff) as usize;
    let mut out = Vec::new();
    let mut at = header;
    for i in 0..count {
        let info = 12 + i * 24;
        let encoded = be(info)? as usize;
        let body = data
            .get(at..at + encoded)
            .ok_or_else(|| bad("BLTE chunk truncated"))?;
        out.extend(chunk(body)?);
        at += encoded;
    }
    Ok(out)
}

fn chunk(body: &[u8]) -> Result<Vec<u8>> {
    let (&mode, rest) = body.split_first().ok_or_else(|| bad("empty BLTE chunk"))?;
    match mode {
        b'N' => Ok(rest.to_vec()),
        b'Z' => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(rest)
                .read_to_end(&mut out)
                .map_err(|e| bad(format!("zlib: {e}")))?;
            Ok(out)
        }
        b'F' => decode(rest),
        b'E' => Err(Error::Encrypted),
        m => Err(bad(format!("BLTE mode {:?}", m as char))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn plain_and_zlib_chunks_decode() {
        let mut one = b"BLTE".to_vec();
        one.extend_from_slice(&0u32.to_be_bytes());
        one.extend_from_slice(b"Nhello");
        assert_eq!(decode(&one).unwrap(), b"hello");

        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(b" world").unwrap();
        let z = [b"Z".as_slice(), &z.finish().unwrap()].concat();
        let n = b"Nhello".to_vec();
        let mut two = b"BLTE".to_vec();
        two.extend_from_slice(&(12u32 + 2 * 24).to_be_bytes());
        two.extend_from_slice(&(0x0F00_0002u32).to_be_bytes());
        for c in [&n, &z] {
            two.extend_from_slice(&(c.len() as u32).to_be_bytes());
            two.extend_from_slice(&[0; 20]);
        }
        two.extend_from_slice(&n);
        two.extend_from_slice(&z);
        assert_eq!(decode(&two).unwrap(), b"hello world");
    }
}
