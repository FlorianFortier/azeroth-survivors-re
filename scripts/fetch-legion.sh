#!/usr/bin/env bash
# Fetch what the native client reads of World of Warcraft: Legion (7.3.5, build 26972): Legion's
# creatures, races, classes and the Broken Isles (docs/EXPANSIONS.md). This repo ships no
# Blizzard content, nor WoWDBDefs' definitions; both are downloaded here, into legion-data/.
#
#   ./scripts/fetch-legion.sh            the install's game files (71 GB of its 72: .build.info,
#                                        Data/config, Data/data), each checked against
#                                        archive.org's SHA-1; resumable, re-runnable
#   ./scripts/fetch-legion.sh --stream   nothing heavy: the game reads the copy on archive.org
#                                        a few bytes at a time as it needs them (curl at run time),
#                                        kept in legion-data/cache
#   ./scripts/fetch-legion.sh --install <folder>
#                                        a Legion 7.3.5.26972 install already on this machine (the
#                                        folder holding .build.info): nothing of the game is
#                                        downloaded, and its path is kept in legion-data/install.path
#
# Either way it fetches the WoWDBDefs definitions the client types Legion's tables with, pinned
# to one commit, into legion-data/dbd. The launcher (`make native`, `make dist`) finds
# legion-data/ next to itself; or set WOW_LEGION, WOW_LEGION_DBD (and WOW_LEGION_CACHE) by hand,
# say to a Legion 7.3.5 install of your own.
#
# Requires: curl, python3, sha1sum (or shasum).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEGION_DIR="${LEGION_DIR:-$ROOT/legion-data}"

# --- Sources ---------------------------------------------------------------------
# The archive.org item holding a Legion 7.3.5.26972 client, and the install's folder in it.
# Override LEGION_URL if the item ever moves; it must name the folder that holds .build.info.
ITEM="lrdpsychochains-legion-7.3.5.26972-client"
DEFAULT_URL="https://archive.org/download/$ITEM/D:/LegionClient/WoW_Legion_7.3.5.26972"
URL="${LEGION_URL:-$DEFAULT_URL}"
FILES_XML_URL="${LEGION_FILES_URL:-https://archive.org/download/$ITEM/${ITEM}_files.xml}"
PREFIX="D:/LegionClient/WoW_Legion_7.3.5.26972/"
# WoWDBDefs, pinned: the layouts of build 7.3.5.26972 the client reads.
DBD_COMMIT="72891e941188b2e0b4a36d42b734434e41ec7fc9"
DBD_URL="https://raw.githubusercontent.com/wowdev/WoWDBDefs/$DBD_COMMIT/definitions"
# The tables `benilla-legion` reads (`library::table`'s names).
TABLES=(
    AnimationData CharHairGeosets CharSections CharacterFacialHairStyles ChrRaces
    ChrSpecialization CreatureDisplayInfo CreatureModelData Item ItemAppearance ItemDisplayInfo
    ItemModifiedAppearance Light LightData LightParams LiquidMaterial LiquidType ModelFileData
    Spell SpellMisc Talent TextureFileData
)

STREAM=0
INSTALL_OWN=""
case "${1:-}" in
    --stream) STREAM=1 ;;
    --install)
        INSTALL_OWN="${2:-}"
        if [ ! -f "$INSTALL_OWN/.build.info" ]; then
            echo "$0: --install needs the folder holding .build.info (got '${INSTALL_OWN}')" >&2
            exit 2
        fi
        ;;
    "") ;;
    *) echo "usage: $0 [--stream | --install <folder>]"; exit 2 ;;
esac

if command -v sha1sum >/dev/null; then SHA1="sha1sum"; else SHA1="shasum -a 1"; fi

mkdir -p "$LEGION_DIR/dbd"

echo "fetching WoWDBDefs' definitions (commit ${DBD_COMMIT:0:12})…"
for t in "${TABLES[@]}"; do
    out="$LEGION_DIR/dbd/$t.dbd"
    [ -s "$out" ] || curl -sSfL -o "$out" "$DBD_URL/$t.dbd"
done

if [ -n "$INSTALL_OWN" ]; then
    # An install of the player's own: only where it is, for the launcher.
    INSTALL_OWN="$(cd "$INSTALL_OWN" && pwd)"
    echo "$INSTALL_OWN" >"$LEGION_DIR/install.path"
    rm -f "$LEGION_DIR/stream.url"
    echo "Legion is read from $INSTALL_OWN"
    echo "  WOW_LEGION=$INSTALL_OWN"
    echo "  WOW_LEGION_DBD=$LEGION_DIR/dbd"
    exit 0
fi

# The item's file list: name, size and SHA-1 of every file the install holds that the client
# reads (.build.info, Data/config, Data/data but its shmem), as `size sha1 path` lines.
LIST="$LEGION_DIR/files.txt"
if [ ! -s "$LIST" ]; then
    echo "reading the archive.org item's file list…"
    curl -sSfL "$FILES_XML_URL" | python3 -c '
import sys, xml.etree.ElementTree as ET
prefix = sys.argv[1]
for f in ET.parse(sys.stdin).getroot().iter("file"):
    name = f.get("name", "")
    if f.get("source") != "original" or not name.startswith(prefix):
        continue
    path = name[len(prefix):]
    keep = path == ".build.info" or path.startswith("Data/config/") or (
        path.startswith("Data/data/") and not path.endswith("/shmem"))
    if keep:
        print(f.findtext("size"), f.findtext("sha1"), path)
' "$PREFIX" >"$LIST.part"
    mv "$LIST.part" "$LIST"
fi

if [ "$STREAM" = 1 ]; then
    # The copy stays on archive.org; the reader needs only Data/data's names to list it.
    CACHE="$LEGION_DIR/cache"
    mkdir -p "$CACHE/Data"
    awk '$3 ~ /^Data\/data\// { sub(/^Data\/data\//, "", $3); print $3 }' "$LIST" >"$CACHE/Data/data.list"
    echo "$URL" >"$LEGION_DIR/stream.url"
    rm -f "$LEGION_DIR/install.path"
    echo "Legion streams from archive.org, cached in $CACHE"
    echo "  WOW_LEGION=$URL"
    echo "  WOW_LEGION_CACHE=$CACHE"
    echo "  WOW_LEGION_DBD=$LEGION_DIR/dbd"
    exit 0
fi

INSTALL="$LEGION_DIR/install"
total=$(awk '{ s += $1 } END { printf "%.1f", s / 1e9 }' "$LIST")
count=$(wc -l <"$LIST")
echo "downloading the Legion install's game files ($count files, $total GB) into $INSTALL…"
n=0
while read -r size sha1 path; do
    n=$((n + 1))
    out="$INSTALL/$path"
    ok="$out.sha1-ok"
    # A file checked once is not hashed again on the next run.
    if [ -f "$ok" ] && [ "$(cat "$ok")" = "$sha1" ] && [ "$(wc -c <"$out")" = "$size" ]; then
        continue
    fi
    mkdir -p "$(dirname "$out")"
    if [ ! -f "$out" ] || [ "$(wc -c <"$out")" != "$size" ]; then
        echo "  [$n/$count] $path ($((size / 1000000)) MB)"
        curl -sSfL --retry 5 -C - -o "$out" "$URL/$path" || {
            # A resume the server refused: start the file over.
            rm -f "$out"
            curl -sSfL --retry 5 -o "$out" "$URL/$path"
        }
    fi
    got=$($SHA1 "$out" | cut -d' ' -f1)
    if [ "$got" != "$sha1" ]; then
        echo "  $path: SHA-1 $got, not $sha1; removed, run again to fetch it anew" >&2
        rm -f "$out" "$ok"
        exit 1
    fi
    echo "$sha1" >"$ok"
done <"$LIST"
rm -f "$LEGION_DIR/stream.url" "$LEGION_DIR/install.path"

echo "Legion's game files are ready at $INSTALL"
echo "  WOW_LEGION=$INSTALL"
echo "  WOW_LEGION_DBD=$LEGION_DIR/dbd"
