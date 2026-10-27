//! Files answered beside the MPQ patch chain, native only: a reader the app registers once (the
//! Legion install, `benilla-legion`'s `Library`) is asked first for every `mpq://` path, and
//! answers the paths that are its own. A path it does not answer goes to the chain as before.

use std::sync::OnceLock;

/// A path's bytes, or `None` for a path that is not the reader's.
pub type ExtraReader = dyn Fn(&str) -> Option<std::io::Result<Vec<u8>>> + Send + Sync;

static EXTRA: OnceLock<Box<ExtraReader>> = OnceLock::new();

/// Register the reader; a second registration is refused, and returns `false`.
pub fn set_extra_source(reader: Box<ExtraReader>) -> bool {
    EXTRA.set(reader).is_ok()
}

/// The registered reader's answer for `path` (lowercase, `/` between folders).
pub(crate) fn read(path: &str) -> Option<std::io::Result<Vec<u8>>> {
    EXTRA.get().and_then(|r| r(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extra_source_answers_only_its_own_paths() {
        assert!(set_extra_source(Box::new(|p| {
            p.strip_prefix("extra/").map(|r| Ok(r.as_bytes().to_vec()))
        })));
        assert!(!set_extra_source(Box::new(|_| None)));
        assert_eq!(read("extra/abc").unwrap().unwrap(), b"abc");
        assert!(read("world/foo.blp").is_none());
    }
}
