//! Tests on a real Legion 7.3.5 install, read remotely: `LEGION_URL` (the install's folder over
//! HTTP) and `LEGION_CACHE` (a local folder for what is fetched). Ignored, as CI has no data:
//!
//! ```text
//! LEGION_URL=https://…/WoW_Legion_7.3.5.26972 LEGION_CACHE=/tmp/legion \
//!   cargo test -p benilla-legion --test legion_data -- --ignored --nocapture
//! ```

use std::io::Cursor;
use std::path::PathBuf;

use benilla_casc::remote::Remote;
use benilla_casc::Casc;
use benilla_legion::library::{AnySource, Library};
use benilla_legion::transcode::{self, virtual_path};
use benilla_legion::{md21, model, skin};

fn casc() -> Casc<Remote> {
    Casc::open(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    })
    .expect("the Legion install")
}

#[test]
#[ignore = "needs a Legion install: LEGION_URL, LEGION_CACHE"]
fn illidan_transcodes_to_a_model_the_client_reads() {
    let casc = casc();
    let file = casc.read_path("creature/illidan2/illidan2.m2").unwrap();
    let chunks = md21::split(&file).unwrap();
    let legion = model::parse(&chunks.md20).unwrap();
    let skin_file = casc.read_id(chunks.skins[0]).unwrap();
    let legion_skin = skin::parse(&skin_file).unwrap();
    let names: Vec<Option<String>> =
        transcode::texture_files(&legion, &chunks.textures, None, |p| casc.id_of(p))
            .into_iter()
            .map(|f| f.map(virtual_path))
            .collect();
    let shows = |id: u16| id == 0 || id % 100 == 1;
    let vanilla = transcode::to_vanilla(
        &chunks.md20,
        &skin_file,
        &names,
        shows,
        &transcode::Animation::default(),
    )
    .unwrap();

    let f = benilla_m2::parse_m2(&mut Cursor::new(&vanilla[..])).unwrap();
    let m = f.model();
    let s = m.parse_embedded_skin(&vanilla, 0).unwrap();
    println!(
        "Illidan: {} vertices, {} bones, {} textures, {} sections, {} of {} batches, {} attachments",
        m.vertices.len(),
        m.bones.len(),
        m.textures.len(),
        s.submeshes().len(),
        s.batches().len(),
        legion_skin.batches.len(),
        m.attachments.len()
    );
    assert_eq!(m.vertices.len(), legion.vertices.len());
    assert_eq!(m.vertices.len(), 6005);
    assert_eq!(m.bones.len(), 127);
    assert_eq!(s.indices().len(), legion_skin.vertices.len());
    assert_eq!(s.triangles().len(), legion_skin.indices.len());
    assert_eq!(s.submeshes().len(), legion_skin.sections.len());
    let kept = legion_skin
        .batches
        .iter()
        .filter(|b| shows(legion_skin.sections[b.section as usize].id))
        .count();
    assert_eq!(s.batches().len(), kept);
    for (t, name) in m.textures.iter().zip(&names) {
        if let Some(name) = name {
            assert_eq!(t.filename.string.to_str().unwrap(), name);
        }
    }

    // The client's own loader builds its draw from it: every batch, the glow over the body too,
    // which is transparent but in two sequences, and so animated, not culled.
    let draws = benilla_formats::parse_m2_render_submeshes(&vanilla, "", &[]).unwrap();
    println!("{} render submeshes", draws.len());
    assert_eq!((kept, draws.len()), (3, 3));
    assert!(draws.iter().all(|d| !d.indices.is_empty()));
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn the_library_answers_the_clients_legion_paths() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    assert!(library.read("world/azeroth/foo.blp").is_none());
    assert!(library.displays().count() > 60_000);

    // The Fel Lord's display, in its own skins, as the client's model loader reads it: eleven
    // draws, one of them a layer transparent at its first key and animated after.
    let model = library.read("legion/display/61830.m2").unwrap().unwrap();
    let draws = benilla_formats::parse_m2_render_submeshes(&model, "", &[]).unwrap();
    assert_eq!(draws.len(), 11);
    let textures: Vec<&str> = draws.iter().filter_map(|d| d.texture.as_deref()).collect();
    assert!(textures.contains(&"Legion\\1089901.blp"), "{textures:?}");
    // Each texture it names is there, a BLP2.
    for t in &textures {
        let path = t.to_ascii_lowercase().replace('\\', "/");
        let blp = library.read(&path).unwrap().unwrap();
        assert_eq!(&blp[..4], b"BLP2", "{path}");
    }
    // The bare model reads too, and a path that names nothing is an error, not a miss.
    assert!(library.read("legion/1089899.m2").unwrap().is_ok());
    assert!(library.read("legion/display/x.m2").unwrap().is_err());
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn illidan_walks_strikes_and_dies_as_the_client_reads_him() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let casc = casc();
    let id = casc.id_of("creature/illidan2/illidan2.m2").unwrap();
    let model = library.read(&format!("legion/{id}.m2")).unwrap().unwrap();

    let clips = benilla_formats::parse_m2_animations(&model);
    let moving = |anim: u16| {
        clips
            .iter()
            .find(|c| c.anim_id == anim)
            .map(|c| c.bones.iter().filter(|b| b.rotation.len() > 1).count())
    };
    for (anim, what) in [
        (0, "stand"),
        (4, "walk"),
        (5, "run"),
        (16, "attack unarmed"),
        (17, "attack one-handed"),
        (1, "death"),
        // In an .anim file of its own.
        (60, "spell kneel"),
        // An alias of 193.
        (108, "an alias"),
    ] {
        let bones = moving(anim).unwrap_or_else(|| panic!("no {what} ({anim})"));
        println!("{what} ({anim}): {bones} bones turn");
        assert!(bones > 10, "{what} ({anim}) moves {bones} bones");
    }
    let death = clips.iter().find(|c| c.anim_id == 1).unwrap();
    assert!(!death.looping, "death holds its last pose");
    assert!(clips.iter().find(|c| c.anim_id == 0).unwrap().looping);

    // An animation he lacks falls back as Legion would: none here is never a bare stand for all.
    let playable = benilla_formats::parse_m2_playable_animation_lookup(&model).unwrap();
    assert!(playable.len() > 500, "{}", playable.len());
    let owns = benilla_formats::parse_m2_animation_lookup(&model).unwrap();
    for (i, p) in playable.iter().enumerate().take(200) {
        let r = p.resolved_id as usize;
        assert!(
            owns.get(r).is_some_and(|&s| s != 0xffff),
            "{i} plays {r}, not his"
        );
    }
}
