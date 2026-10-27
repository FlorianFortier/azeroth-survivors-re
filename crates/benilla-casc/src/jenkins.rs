//! Bob Jenkins' `hashlittle2` (lookup3), which root keys a file's path by: the path in upper case
//! with `\` for `/`, both 32-bit results joined (`c` high, `b` low), both seeds 0.

fn rot(x: u32, k: u32) -> u32 {
    x.rotate_left(k)
}

fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= rot(*c, 4);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= rot(*a, 6);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= rot(*b, 8);
    *b = b.wrapping_add(*a);
    *a = a.wrapping_sub(*c);
    *a ^= rot(*c, 16);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= rot(*a, 19);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= rot(*b, 4);
    *b = b.wrapping_add(*a);
}

fn fin(a: &mut u32, b: &mut u32, c: &mut u32) {
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 14));
    *a ^= *c;
    *a = a.wrapping_sub(rot(*c, 11));
    *b ^= *a;
    *b = b.wrapping_sub(rot(*a, 25));
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 16));
    *a ^= *c;
    *a = a.wrapping_sub(rot(*c, 4));
    *b ^= *a;
    *b = b.wrapping_sub(rot(*a, 14));
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 24));
}

/// `hashlittle2(key, pc = 0, pb = 0)`: `(c, b)`.
pub fn hashlittle2(key: &[u8]) -> (u32, u32) {
    let init = 0xdead_beefu32.wrapping_add(key.len() as u32);
    let (mut a, mut b, mut c) = (init, init, init);
    let word = |s: &[u8], i: usize| -> u32 {
        let mut w = [0u8; 4];
        let n = s.len().saturating_sub(i).min(4);
        w[..n].copy_from_slice(&s[i..i + n]);
        u32::from_le_bytes(w)
    };
    let mut k = key;
    while k.len() > 12 {
        a = a.wrapping_add(word(k, 0));
        b = b.wrapping_add(word(k, 4));
        c = c.wrapping_add(word(k, 8));
        mix(&mut a, &mut b, &mut c);
        k = &k[12..];
    }
    if k.is_empty() {
        return (c, b);
    }
    a = a.wrapping_add(word(k, 0));
    if k.len() > 4 {
        b = b.wrapping_add(word(k, 4));
    }
    if k.len() > 8 {
        c = c.wrapping_add(word(k, 8));
    }
    fin(&mut a, &mut b, &mut c);
    (c, b)
}

/// The root's hash of a path.
pub fn path_hash(path: &str) -> u64 {
    let norm: Vec<u8> = path
        .bytes()
        .map(|b| match b {
            b'/' => b'\\',
            b => b.to_ascii_uppercase(),
        })
        .collect();
    let (c, b) = hashlittle2(&norm);
    u64::from(c) << 32 | u64::from(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup3s_own_test_vectors() {
        // From lookup3.c's driver5: hashlittle2("", 0, 0) and "Four score and seven years ago".
        assert_eq!(hashlittle2(b""), (0xdeadbeef, 0xdeadbeef));
        assert_eq!(
            hashlittle2(b"Four score and seven years ago"),
            (0x17770551, 0xce7226e6)
        );
    }
}
