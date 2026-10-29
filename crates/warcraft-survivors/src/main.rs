//! The `warcraft-survivors` launcher: the build stamp, handed to the survivors mode, and the game
//! data found beside it ([`find_data`]).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use benilla_app::BuildId;

fn main() -> benilla_app::AppExit {
    find_data();
    benilla_app::run_survivors(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: env!("BENILLA_PROJECT_DIR"),
        ..Default::default()
    })
}

/// The data the fetch scripts put down, found where they put it: `wow-data/Data`
/// (`scripts/fetch-data.sh`) and `legion-data/` (`scripts/fetch-legion.sh`: its `install/`, the
/// `install.path` naming an install of the player's own, or the `stream.url` and `cache/` of a
/// copy read from archive.org, and its `dbd/`), beside the executable (a `make dist` folder) or
/// in the folder it runs from (`make native`). A variable already set wins, so a player's own
/// install is named as before.
fn find_data() {
    let bases: Vec<PathBuf> = [
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf)),
        std::env::current_dir().ok(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let found = |rel: &str| bases.iter().map(|b| b.join(rel)).find(|p| p.exists());

    if let Some(data) = found("wow-data/Data") {
        set_default("WOW_DATA", data.as_os_str());
    }
    if std::env::var_os("WOW_LEGION").is_none() {
        if let Some((legion, cache)) = legion_source(&bases) {
            set_default("WOW_LEGION", &legion);
            if let Some(cache) = cache {
                set_default("WOW_LEGION_CACHE", &cache);
            }
        }
    }
    if let Some(dbd) = found("legion-data/dbd") {
        set_default("WOW_LEGION_DBD", dbd.as_os_str());
    }
}

/// Where Legion is read from, as `scripts/fetch-legion.sh` left it under a `legion-data/` of
/// `bases`: its `install/` folder, else the folder `install.path` names (an install of the
/// player's own), else `stream.url`'s copy with its `cache/`.
fn legion_source(bases: &[PathBuf]) -> Option<(OsString, Option<OsString>)> {
    let found = |rel: &str| bases.iter().map(|b| b.join(rel)).find(|p| p.exists());
    if let Some(info) = found("legion-data/install/.build.info") {
        return Some((info.parent().unwrap_or(&info).as_os_str().to_owned(), None));
    }
    let text = |p: PathBuf| {
        std::fs::read_to_string(p)
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    };
    if let Some(own) = found("legion-data/install.path").and_then(text) {
        return Some((own.into(), None));
    }
    let url = found("legion-data/stream.url")?;
    let cache = url.parent().map(|d| d.join("cache").into_os_string());
    Some((text(url)?.into(), cache))
}

/// Set `key` to `value` unless it is set already. Called before anything else runs, so before
/// any thread reads the environment.
fn set_default(key: &str, value: &std::ffi::OsStr) {
    if std::env::var_os(key).is_none() {
        eprintln!("warcraft-survivors: {key}={}", value.to_string_lossy());
        std::env::set_var(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legion_is_found_as_the_fetch_script_left_it() {
        let tmp = std::env::temp_dir().join(format!("survivors-launcher-{}", std::process::id()));
        let legion = tmp.join("legion-data");
        std::fs::create_dir_all(&legion).unwrap();
        let bases = vec![tmp.clone()];
        assert_eq!(legion_source(&bases), None);
        // A streamed copy: its URL and its cache beside it.
        std::fs::write(legion.join("stream.url"), "https://example.org/legion\n").unwrap();
        let (url, cache) = legion_source(&bases).unwrap();
        assert_eq!(url, OsString::from("https://example.org/legion"));
        assert_eq!(cache, Some(legion.join("cache").into_os_string()));
        // An install of the player's own, named by its path, wins over the stream.
        std::fs::write(
            legion.join("install.path"),
            "D:\\WoW-Legion\\WoW_Legion_7.3.5.26972\n",
        )
        .unwrap();
        let (own, cache) = legion_source(&bases).unwrap();
        assert_eq!(
            own,
            OsString::from("D:\\WoW-Legion\\WoW_Legion_7.3.5.26972")
        );
        assert_eq!(cache, None);
        // A full install fetched here wins over both.
        std::fs::create_dir_all(legion.join("install")).unwrap();
        std::fs::write(legion.join("install/.build.info"), "").unwrap();
        let (dir, _) = legion_source(&bases).unwrap();
        assert_eq!(dir, legion.join("install").into_os_string());
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
