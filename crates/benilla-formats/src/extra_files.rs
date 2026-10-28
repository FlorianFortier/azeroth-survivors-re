//! Files answered beside the MPQ patch chain, native only: a source the app registers once (the
//! Legion install, `benilla-legion`'s `Library`) owns some paths, and [`crate::Chain`] reads those
//! from it, so every reader of the chain (the `mpq://` asset reader, the body compositor) sees
//! them. A path it does not own goes to the archives as before.

use std::sync::OnceLock;

/// A source of files beside the archives.
pub trait ExtraFiles: Send + Sync {
    /// Whether `path` (lowercase, `/` between folders) is one of its own, without reading it.
    fn claims(&self, path: &str) -> bool;
    /// The file at `path`, one it claims.
    fn read(&self, path: &str) -> std::io::Result<Vec<u8>>;
}

static EXTRA: OnceLock<Box<dyn ExtraFiles>> = OnceLock::new();

/// Register the source; a second registration is refused, and returns `false`.
pub fn set_extra_files(files: Box<dyn ExtraFiles>) -> bool {
    EXTRA.set(files).is_ok()
}

/// A chain name as the source reads it: lowercase, `/` between folders.
fn key(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

/// The source's file at `name` (`/` or `\`, any case), `None` when it does not own it.
pub fn extra_file(name: &str) -> Option<std::io::Result<Vec<u8>>> {
    let files = EXTRA.get()?;
    let key = key(name);
    files.claims(&key).then(|| files.read(&key))
}

/// Whether the source owns `name`.
pub(crate) fn has_extra_file(name: &str) -> bool {
    EXTRA.get().is_some_and(|f| f.claims(&key(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Extra;

    impl ExtraFiles for Extra {
        fn claims(&self, path: &str) -> bool {
            path.starts_with("extra/")
        }
        fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
            Ok(path.as_bytes().to_vec())
        }
    }

    #[test]
    fn the_extra_source_answers_only_its_own_paths() {
        assert!(set_extra_files(Box::new(Extra)));
        assert!(!set_extra_files(Box::new(Extra)));
        assert_eq!(extra_file("Extra\\Abc").unwrap().unwrap(), b"extra/abc");
        assert!(has_extra_file("EXTRA/x") && !has_extra_file("World\\foo.blp"));
        assert!(extra_file("World\\foo.blp").is_none());
    }
}
