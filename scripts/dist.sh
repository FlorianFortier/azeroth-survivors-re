#!/usr/bin/env bash
# Build the native game for players into dist/azeroth-survivors/: the release executable (a player
# build, no `dev` seams), the two fetch scripts, and a README. No game data goes in: a player
# fetches it beside the executable (scripts/fetch-data.sh, scripts/fetch-legion.sh), where the
# executable finds it, or names an install of their own (WOW_DATA, WOW_LEGION, WOW_LEGION_DBD).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${DIST_DIR:-$ROOT/dist}/azeroth-survivors"
BIN=warcraft-survivors
EXE="$BIN"
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) EXE="$BIN.exe" ;; esac

echo "building the player build (release, without dev)…"
cargo build --release -p "$BIN" --no-default-features --manifest-path "$ROOT/Cargo.toml"

rm -rf "$OUT"
mkdir -p "$OUT/scripts"
cp "$ROOT/target/release/$EXE" "$OUT/"
cp "$ROOT/scripts/fetch-data.sh" "$ROOT/scripts/fetch-legion.sh" "$OUT/scripts/"
cat >"$OUT/README.txt" <<'TXT'
Azeroth Survivors, native build
===============================

No World of Warcraft data comes with this folder. Fetch it beside the executable, once:

  ./scripts/fetch-data.sh               a WoW 1.12.1 client's Data folder (5.5 GB) into wow-data/
  ./scripts/fetch-legion.sh             optional: Legion 7.3.5's game files (71 GB, each file
                                        checked) into legion-data/, for Legion's creatures, races,
                                        classes and Val'sharah
  ./scripts/fetch-legion.sh --stream    or: no big download, Legion read from archive.org as the
                                        game needs it (curl, and the network, at play time)
  ./scripts/fetch-legion.sh --install <folder>
                                        or: a Legion 7.3.5.26972 install already here (the folder
                                        holding .build.info), remembered in legion-data/

The scripts need curl, unzip, python3 and sha1sum (or shasum). Then run ./warcraft-survivors:
it finds wow-data/ and legion-data/ beside itself, or in the folder it is run from. To use an
install of your own instead, set WOW_DATA (its Data folder), WOW_LEGION (a Legion 7.3.5 folder)
and WOW_LEGION_DBD (WoWDBDefs' .dbd definitions, as fetch-legion.sh puts in legion-data/dbd).

Without Legion's data the game plays as 1.12.1 alone: Legion's races, classes and zone are not
offered, and Legion's creatures wear their 1.12.1 looks.
TXT

echo "ready: $OUT"
ls -la "$OUT"
