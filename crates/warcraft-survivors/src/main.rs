//! The `warcraft-survivors` launcher: the build stamp, handed to the survivors mode, and the game
//! data found beside it ([`find_data`]).

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
/// (`scripts/fetch-data.sh`) and `legion-data/` (`scripts/fetch-legion.sh`: its `install/`, or
/// the `stream.url` and `cache/` of a copy read from archive.org, and its `dbd/`), beside the
/// executable (a `make dist` folder) or in the folder it runs from (`make native`). A variable
/// already set wins, so a player's own install is named as before.
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
        if let Some(info) = found("legion-data/install/.build.info") {
            set_default("WOW_LEGION", info.parent().unwrap_or(&info).as_os_str());
        } else if let Some(url) = found("legion-data/stream.url") {
            if let Ok(text) = std::fs::read_to_string(&url) {
                set_default("WOW_LEGION", text.trim().as_ref());
                if let Some(dir) = url.parent() {
                    set_default("WOW_LEGION_CACHE", dir.join("cache").as_os_str());
                }
            }
        }
    }
    if let Some(dbd) = found("legion-data/dbd") {
        set_default("WOW_LEGION_DBD", dbd.as_os_str());
    }
}

/// Set `key` to `value` unless it is set already. Called before anything else runs, so before
/// any thread reads the environment.
fn set_default(key: &str, value: &std::ffi::OsStr) {
    if std::env::var_os(key).is_none() {
        eprintln!("warcraft-survivors: {key}={}", value.to_string_lossy());
        std::env::set_var(key, value);
    }
}
