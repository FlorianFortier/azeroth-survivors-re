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
        transcode::texture_files(&legion, &chunks.textures, |_| None, |p| casc.id_of(p))
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

#[test]
#[ignore = "needs a Legion install: LEGION_URL, LEGION_CACHE"]
fn fel_fire_trails_and_glows_read_as_the_clients() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let library = Library::open(source, None).unwrap();
    let casc = casc();
    let model = |path: &str| {
        let id = casc.id_of(path).unwrap();
        library.read(&format!("legion/{id}.m2")).unwrap().unwrap()
    };

    // A fel fireball: five emitters, fel green.
    let fireball = model("spells/fel_fireball_missile_high.m2");
    let ps = benilla_formats::parse_m2_particle_emitters(&fireball).unwrap();
    println!(
        "fel fireball: {} emitters, {:?}",
        ps.len(),
        ps.iter()
            .map(|p| (p.blend, p.shape, p.texture.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(ps.len(), 5);
    assert!(ps.iter().all(|p| p.texture.is_some()));
    // Four glows add, the fifth, its smoke, blends.
    let adds = ps
        .iter()
        .filter(|p| p.blend == benilla_formats::ParticleBlend::Add)
        .count();
    assert_eq!(adds, 4);
    // Its green: more green than red at mid-life on its second emitter.
    let c = ps[1].over_life.color[1];
    assert!(c[1] > c[0], "{c:?}");

    // A trail: one ribbon, textured.
    let trail = model("spells/ribbontrail.m2");
    let rs = benilla_formats::parse_m2_ribbon_emitters(&trail).unwrap();
    assert_eq!(rs.len(), 1);
    assert!(rs[0].texture.is_some());
    assert_eq!(rs[0].edges_per_second, 50.0);

    // The HD infernal's burning skin: a texture transform, keyed.
    let infernal = model("creature/infernal2/infernal2.m2");
    let f = benilla_m2::parse_m2(&mut Cursor::new(&infernal[..])).unwrap();
    let t = &f.model().texture_transforms;
    println!(
        "infernal2: {} texture transforms, translation keys {}",
        t.len(),
        t[0].translation.keys.len()
    );
    assert_eq!(t.len(), 1);
    assert!(t[0].translation.keys.len() + t[0].rotation.keys.len() + t[0].scaling.keys.len() > 1);
    assert!(f.model().texture_transform_lookup.contains(&0));
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn legion_weapons_answer_where_the_client_looks_for_them() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();

    // Illidan's Warglaive of Azzinoth (item 32837): display 45150, its model and skin.
    let glaive = library.item_look(45150).unwrap();
    assert_eq!((glaive.models[0], glaive.textures[0]), (145562, 145561));
    let path = format!(
        "item/objectcomponents/weapon/{}",
        benilla_legion::library::item_model_name(45150, 0)
    );
    let model = library.read(&path).unwrap().unwrap();
    let draws = benilla_formats::parse_m2_render_submeshes(&model, "", &[]).unwrap();
    assert!(!draws.is_empty());
    assert!(
        draws
            .iter()
            .any(|d| d.texture.as_deref() == Some("Legion\\145561.blp")),
        "{:?}",
        draws.iter().map(|d| d.texture.clone()).collect::<Vec<_>>()
    );

    // Legion's own looks, four a family, the same list every time.
    let families = [(2, 7), (2, 10), (2, 0), (2, 15), (4, 6)];
    let looks = library.weapon_looks(&families, 4);
    println!(
        "{:?}",
        looks
            .iter()
            .map(|l| (l.id, l.class, l.subclass, l.models[0]))
            .collect::<Vec<_>>()
    );
    assert_eq!(looks.len(), 20);
    assert_eq!(looks, library.weapon_looks(&families, 4));
    for l in &looks {
        assert!(l.models[0] >= 1_100_000 && l.textures[0] != 0, "{l:?}");
        let model = library.item_model(l.id, 0).unwrap();
        assert!(!benilla_formats::parse_m2_render_submeshes(&model, "", &[])
            .unwrap()
            .is_empty());
    }
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn legions_races_read_as_1_12_1_races() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    for (id, file, models, horde) in [
        (10u8, "BloodElf", [117170, 116921], true),
        (11, "Draenei", [117721, 117437], false),
        (9, "Goblin", [119376, 119369], true),
        (22, "Worgen", [307454, 307453], false),
    ] {
        let race = library.race(id).unwrap();
        println!(
            "{file}: {} sections, {} hair, {} facial",
            race.sections.len(),
            race.hair.len(),
            race.facial.len()
        );
        assert_eq!(
            (race.file.as_str(), race.models, race.horde),
            (file, models, horde)
        );
        for sex in [0u8, 1] {
            // A base skin for each of several colours, faces, hair styles.
            let skins = race
                .sections
                .iter()
                .filter(|((s, b, v, _), t)| *s == sex && *b == 0 && *v == 0 && t[0] != 0)
                .count();
            assert!(skins >= 5, "{file} {sex}: {skins} skins");
            assert!(race.hair.iter().filter(|((s, _), _)| *s == sex).count() >= 5);
            // The body reads as a 1.12.1 model, every geoset kept.
            let path = format!(
                "character/{}/{}/legion_char_{}.m2",
                file.to_lowercase(),
                if sex == 0 { "male" } else { "female" },
                race.models[usize::from(sex)]
            );
            assert!(library.claims(&path));
            let model = library.read(&path).unwrap().unwrap();
            let draws = benilla_formats::parse_m2_render_submeshes(&model, "", &[]).unwrap();
            assert!(draws.len() > 20, "{path}: {} draws", draws.len());
        }
        // Its skin, at 1.12.1's size.
        let skin = race
            .sections
            .iter()
            .find(|((s, b, v, c), _)| (*s, *b, *v, *c) == (0, 0, 0, 0))
            .unwrap()
            .1[0];
        let half = library
            .read(&format!("legion/half/{skin}.blp"))
            .unwrap()
            .unwrap();
        let blp = benilla_blp::decode(&half).unwrap();
        assert_eq!((blp.width, blp.height), (256, 256));
    }
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn legions_classes_read_with_their_spells_and_talents() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let obliterate = library.spell(49020).unwrap();
    assert_eq!(
        (obliterate.name.as_str(), obliterate.icon),
        ("Obliterate", 135771)
    );
    // Its icon is a texture the library answers.
    let icon = library
        .read(&format!("legion/{}.blp", obliterate.icon))
        .unwrap()
        .unwrap();
    assert!(benilla_blp::decode(&icon).is_ok());
    for (class, names) in [
        (6u8, &["Blood", "Frost", "Unholy"][..]),
        (10, &["Brewmaster", "Mistweaver", "Windwalker"]),
        (12, &["Havoc", "Vengeance"]),
    ] {
        let specs = library.specs(class);
        let got: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(got, names);
        for s in &specs {
            println!("{}: {} talents", s.name, s.talents.len());
            assert_eq!(s.talents.len(), 21, "{}", s.name);
            assert!(s.icon != 0);
            for t in &s.talents {
                assert!(t.tier < 7 && t.column < 3, "{t:?}");
                let spell = library.spell(t.spell).unwrap();
                assert!(!spell.name.is_empty() && spell.icon != 0, "{spell:?}");
            }
        }
    }
}
