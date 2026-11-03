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
        // The Blood Elf in its HD body (`races::HD_RACES`), its old one gutted.
        (10u8, "BloodElf", [1100087, 1100258], true),
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
            // Its skeleton whole: few roots, the arms' key bones there (the Blood Elf's HD body
            // out of its `.skel`).
            let skeleton = benilla_formats::parse_m2_skeleton(&model).unwrap();
            let roots = skeleton.bones.iter().filter(|b| b.parent < 0).count();
            let arms = skeleton.bones.iter().any(|b| (0..=3).contains(&b.key_bone));
            println!("{path}: {} bones, {roots} roots", skeleton.bones.len());
            assert!(
                roots < skeleton.bones.len() / 4 && arms,
                "{path}: {roots} roots"
            );
        }
        // Its skin, at 1.12.1's size.
        let skin = race
            .sections
            .iter()
            .find(|((s, b, v, c), _)| (*s, *b, *v, *c) == (0, 0, 0, 0))
            .unwrap()
            .1[0];
        let atlas = if race.layout == 2 {
            format!("legion/layout2/skin/{skin}.blp")
        } else {
            format!("legion/half/{skin}.blp")
        };
        let blp = benilla_blp::decode(&library.read(&atlas).unwrap().unwrap()).unwrap();
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

#[test]
#[ignore = "needs a Legion install: LEGION_URL, LEGION_CACHE"]
fn the_broken_isles_read_as_a_1_12_1_map() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let library = Library::open(source, None).unwrap();
    let dir = "world/maps/legionbrokenisles/legionbrokenisles";
    // The tile table, as 1.12.1's reader takes it.
    assert!(library.claims(&format!("{dir}.wdt")));
    let wdt = library.read(&format!("{dir}.wdt")).unwrap().unwrap();
    let wdt = benilla_wdt::WdtReader::new(Cursor::new(&wdt[..]), benilla_wdt::WowVersion::Classic)
        .read()
        .unwrap();
    let tiles = (0..64)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .filter(|&(x, y)| wdt.get_tile(x, y).is_some_and(|t| t.has_adt))
        .count();
    assert_eq!(tiles, 729);
    // A tile of Suramar, through the client's own terrain mesher.
    let path = format!("{dir}_24_30.adt");
    assert!(library.claims(&path));
    let adt = library.read(&path).unwrap().unwrap();
    let mesh = benilla_formats::adt_to_tile_mesh(&adt).unwrap();
    println!(
        "{path}: {} chunks, {} vertices, {} doodads",
        mesh.chunks.len(),
        mesh.vertex_count(),
        mesh.doodads.len()
    );
    assert_eq!(mesh.chunks.len(), 256);
    let layered = mesh.chunks.iter().filter(|c| c.alpha_map.is_some()).count();
    assert!(layered > 100, "{layered} chunks with layers over the base");
    // Every texture is one the library answers, as a BLP the client decodes.
    let mut textures: Vec<&String> = mesh
        .chunks
        .iter()
        .flat_map(|c| c.layer_textures.iter())
        .collect();
    textures.sort();
    textures.dedup();
    assert!(textures.len() > 4);
    for t in &textures {
        let p = t.to_lowercase().replace('\\', "/");
        assert!(p.starts_with("legion/"), "{t}");
        let blp = library.read(&p).unwrap().unwrap();
        benilla_blp::decode(&blp).unwrap();
    }
    // The heights stand where Suramar's are, the chunks' corners on the tile.
    let (x0, y0) = benilla_wdt::tile_to_world(24, 30);
    for c in &mesh.chunks {
        for p in &c.positions {
            assert!(p[2].abs() < 2000.0, "{p:?}");
        }
    }
    println!(
        "tile corner {x0} {y0}, first vertex {:?}",
        mesh.chunks[0].positions[0]
    );
    // Its doodads, models the client reads.
    assert!(mesh.doodads.len() > 50);
    let mut models: Vec<&String> = mesh.doodads.iter().map(|d| &d.model).collect();
    models.sort();
    models.dedup();
    let mut failed = Vec::new();
    for m in &models {
        let p = m.to_lowercase().replace('\\', "/").replace(".mdx", ".m2");
        let read = library.read(&p).unwrap();
        if read.is_err() || benilla_m2::parse_m2(&mut Cursor::new(&read.unwrap()[..])).is_err() {
            failed.push(p);
        }
    }
    println!(
        "{} models, {} failed: {failed:?}",
        models.len(),
        failed.len()
    );
    assert!(failed.is_empty());
    // Its buildings, through the client's own WMO readers: each root, its groups' render batches
    // with their textures, its doodads.
    let mut wmos: Vec<&String> = mesh.wmos.iter().map(|w| &w.model).collect();
    wmos.sort();
    wmos.dedup();
    println!(
        "{} placed buildings, {} distinct",
        mesh.wmos.len(),
        wmos.len()
    );
    assert!(!wmos.is_empty());
    let (mut groups, mut batches, mut missing) = (0, 0, 0);
    let mut wmo_textures = std::collections::BTreeSet::new();
    for w in &wmos {
        let p = w.to_lowercase().replace('\\', "/");
        let root = library.read(&p).unwrap().unwrap();
        let root = benilla_formats::parse_wmo_root(&root).unwrap();
        let stem = p.strip_suffix(".wmo").unwrap();
        for gi in 0..root.group_count() {
            let Some(Ok(g)) = library.read(&format!("{stem}_{gi:03}.wmo")) else {
                missing += 1;
                continue;
            };
            let subs = benilla_formats::wmo_group_submeshes(&g, &root).unwrap();
            groups += 1;
            batches += subs.len();
            for s in &subs {
                assert!(!s.positions.is_empty() && s.uvs.len() == s.positions.len());
                let t = s.texture.as_deref().unwrap_or("");
                assert!(
                    t.is_empty() || t.to_lowercase().starts_with("legion"),
                    "{t}"
                );
                if !t.is_empty() {
                    wmo_textures.insert(t.to_lowercase().replace('\\', "/"));
                }
            }
        }
        for d in root.doodads().iter().filter(|d| !d.model.is_empty()) {
            assert!(d.model.starts_with("Legion\\"), "{}", d.model);
        }
    }
    println!(
        "{groups} groups ({missing} without data), {batches} batches, {} textures",
        wmo_textures.len()
    );
    for t in &wmo_textures {
        benilla_blp::decode(&library.read(t).unwrap().unwrap()).unwrap();
    }
    assert!(groups >= wmos.len() && batches > groups);
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn the_broken_isles_water_and_light_read_as_1_12_1s() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let dir = "world/maps/legionbrokenisles/legionbrokenisles";
    // A lake in the hills and the open sea, through the client's own liquid mesher.
    for (x, y, kind) in [
        (22, 26, benilla_formats::LiquidKind::Still),
        (30, 22, benilla_formats::LiquidKind::Ocean),
    ] {
        let adt = library
            .read(&format!("{dir}_{x}_{y}.adt"))
            .unwrap()
            .unwrap();
        let mesh = benilla_formats::adt_to_tile_mesh(&adt).unwrap();
        let wet: Vec<_> = mesh
            .chunks
            .iter()
            .filter(|c| !c.liquids.is_empty())
            .collect();
        assert!(wet.len() > 30, "{x}_{y}: {} chunks with water", wet.len());
        // The water stands over the ground where it is drawn: each wet cell's corners against
        // the ground's at the same corners (the outer grid, row × 17 + column), and against the
        // ground's with rows and columns swapped, which a wrong orientation would match better.
        let (mut corners, mut over, mut over_swapped) = (0, 0, 0);
        for c in &wet {
            let l = &c.liquids[0];
            assert_eq!(l.kind, kind);
            for row in 0..8 {
                for col in 0..8 {
                    if !l.wet[row * 8 + col] {
                        continue;
                    }
                    for (r, k) in [
                        (row, col),
                        (row + 1, col),
                        (row, col + 1),
                        (row + 1, col + 1),
                    ] {
                        let water = l.positions[r * 9 + k][2];
                        corners += 1;
                        over += usize::from(water + 0.5 >= c.positions[r * 17 + k][2]);
                        over_swapped += usize::from(water + 0.5 >= c.positions[k * 17 + r][2]);
                    }
                }
            }
        }
        println!(
            "{x}_{y}: {} chunks with water, corners over their ground {over} of {corners} \
             ({over_swapped} if turned)",
            wet.len()
        );
        // A shore cell has corners on the land: four in five, not all.
        assert!(over * 5 >= corners * 4 && over > over_swapped);
    }
    // The map's light: its global and spheres, each params' eighteen colours and its fog.
    let (lights, params) = library.map_lights(1220).unwrap();
    println!("{} lights, {} params", lights.len(), params.len());
    assert!(lights.iter().any(|l| l.global));
    assert!(lights.len() > 100 && params.len() > 50);
    for p in &params {
        assert!(p.int_bands.iter().all(|b| !b.is_empty()), "params {}", p.id);
        assert!(p.float_bands[0].iter().all(|&(_, e)| e >= 1000.0));
    }
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn valsharahs_field_and_creatures_read() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    // The survivors mode's Val'sharah stands at (2330, 6480, 147): the Field of Dreamers, its
    // ground there about that high and the tile dry around it.
    let (cx, cy, cz) = (2330.0f32, 6480.0f32, 147.0f32);
    let tile = "world/maps/legionbrokenisles/legionbrokenisles_19_27.adt";
    let mesh = benilla_formats::adt_to_tile_mesh(&library.read(tile).unwrap().unwrap()).unwrap();
    let near: Vec<f32> = mesh
        .chunks
        .iter()
        .flat_map(|c| &c.positions)
        .filter(|p| (p[0] - cx).abs() < 20.0 && (p[1] - cy).abs() < 20.0)
        .map(|p| p[2])
        .collect();
    assert!(!near.is_empty());
    let (lo, hi) = near
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &z| (l.min(z), h.max(z)));
    println!("ground at the centre: {lo:.1}..{hi:.1}");
    assert!((lo - cz).abs() < 15.0 && hi - lo < 15.0);
    // The zone's creatures and bosses (`survivors::data::MAPS`' Val'sharah), each a display
    // whose model the client reads.
    for id in [
        1824u32, 70440, 69316, 64281, 936, 69207, 67790, 69714, 68972, 69487, 69105, 69099, 64282,
        69776, 64777, 69825, 67428, 69879, 69474, 64778, 68127, 67745, 65636, 68730,
    ] {
        let d = library.display(id).unwrap();
        let model = library
            .read(&format!("legion/display/{id}.m2"))
            .unwrap()
            .unwrap();
        benilla_m2::parse_m2(&mut Cursor::new(&model[..])).unwrap();
        assert!(d.model_file != 0);
    }
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn the_pandaren_reads_onto_1_12_1s_atlas() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let race = library.race(24).unwrap();
    assert_eq!((race.file.as_str(), race.layout), ("Pandaren", 2));
    println!(
        "Pandaren: {} sections, {} hair, {} facial",
        race.sections.len(),
        race.hair.len(),
        race.facial.len()
    );
    for sex in [0u8, 1] {
        let at = |section: u8, col: usize| {
            race.sections
                .iter()
                .find(|((s, b, v, c), t)| (*s, *b, *v, *c) == (sex, section, 0, 0) && t[col] != 0)
                .map(|(_, t)| t[col])
        };
        // Its skin, the 1024×512 layout, as 1.12.1's 256² atlas.
        let skin = at(0, 0).unwrap();
        let atlas = library
            .read(&format!("legion/layout2/skin/{skin}.blp"))
            .unwrap()
            .unwrap();
        let a = benilla_blp::decode(&atlas).unwrap();
        assert_eq!((a.width, a.height), (256, 256));
        // Its face, the square, as the two face tiles.
        let face = at(1, 1).or(at(1, 0)).unwrap();
        for (half, h) in [("upper", 32), ("lower", 64)] {
            let tile = library
                .read(&format!("legion/layout2/face/{face}_{half}.blp"))
                .unwrap()
                .unwrap();
            let t = benilla_blp::decode(&tile).unwrap();
            assert_eq!((t.width, t.height), (128, h));
        }
        // Its body, its skin's coordinates on the atlas: every one in it, some in the face block.
        let path = format!(
            "character/pandaren/{}/legion_char_{}.m2",
            if sex == 0 { "male" } else { "female" },
            race.models[usize::from(sex)]
        );
        let model = library.read(&path).unwrap().unwrap();
        let draws = benilla_formats::parse_m2_render_submeshes(&model, "", &[]).unwrap();
        assert!(draws.len() > 20, "{path}: {} draws", draws.len());
        let f = benilla_m2::parse_m2(&mut Cursor::new(&model[..])).unwrap();
        let uvs: Vec<[f32; 2]> = f
            .model()
            .vertices
            .iter()
            .map(|v| [v.tex_coords.x, v.tex_coords.y])
            .collect();
        let in_face = uvs
            .iter()
            .filter(|[u, v]| *u <= 0.5 && (0.62..=1.0).contains(v))
            .count();
        println!(
            "{path}: {} vertices, {in_face} in the face block",
            uvs.len()
        );
        assert!(in_face > 50);
    }
}

#[test]
#[ignore = "needs a Legion install: LEGION_URL, LEGION_CACHE"]
fn an_hd_body_reads_its_skeleton_out_of_its_skel() {
    let casc = casc();
    // The Blood Elf's HD bodies (`ChrRaces`' `HighResMaleDisplayID`/`…Female…`, 62127/62128).
    for file in [1100087u32, 1100258] {
        let chunks = md21::split(&casc.read_id(file).unwrap()).unwrap();
        assert_eq!(model::parse(&chunks.md20).unwrap().bones.len(), 0);
        let skel =
            benilla_legion::skel::parse(&casc.read_id(chunks.skel.unwrap()).unwrap()).unwrap();
        assert!(skel.parent.is_none() && skel.anims.len() > 40);
        let md20 = benilla_legion::skel::merge(&chunks.md20, &skel).unwrap();
        let m = model::parse(&md20).unwrap();
        // A whole skeleton this time: few roots, the arms' key bones there.
        let roots = m.bones.iter().filter(|b| b.parent < 0).count();
        assert!(
            m.bones.len() > 200 && roots < m.bones.len() / 4,
            "{roots} roots"
        );
        assert!(m.bones.iter().any(|b| b.key_bone == 0) && m.bones.iter().any(|b| b.key_bone == 3));
        assert!(m.sequences > 300);

        // The body alone (group 0's skin), as the client reads it: its run keyed from an .anim,
        // its stand from the file.
        let files: Vec<(u16, u16, Vec<u8>)> = skel
            .anims
            .iter()
            .filter(|a| a.2 != 0)
            .map(|&(id, v, f)| {
                (
                    id,
                    v,
                    benilla_legion::skel::anim_bones(&casc.read_id(f).unwrap()),
                )
            })
            .collect();
        let animation = transcode::Animation {
            files: &files,
            ..Default::default()
        };
        let skin = casc.read_id(chunks.skins[0]).unwrap();
        let vanilla = transcode::to_vanilla(&md20, &skin, &[], |id| id == 0, &animation).unwrap();
        let skeleton = benilla_formats::parse_m2_skeleton(&vanilla).unwrap();
        assert_eq!(skeleton.bones.len(), m.bones.len());
        let clips = benilla_formats::parse_m2_animations(&vanilla);
        let turning = |anim: u16| {
            clips.iter().find(|c| c.anim_id == anim).map_or(0, |c| {
                c.bones.iter().filter(|b| b.rotation.len() > 1).count()
            })
        };
        let in_anim = skel.anims.iter().map(|a| a.0).find(|&a| turning(a) > 10);
        println!(
            "{file}: {} bones, {roots} roots, stand turns {}, run {}, from an .anim: {in_anim:?}",
            m.bones.len(),
            turning(0),
            turning(5)
        );
        assert!(turning(0) > 10 && turning(5) > 10);
        assert!(in_anim.is_some(), "no .anim sequence moves");
    }
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions (ZoneLight, ZoneLightPoint): LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn valsharah_takes_its_zones_light() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let (lights, params) = library.map_lights(1220).unwrap();
    use benilla_legion::lights::ZONE_LIGHT_BASE;
    let zones: Vec<_> = lights.iter().filter(|l| l.id >= ZONE_LIGHT_BASE).collect();
    println!("{} zone cells", zones.len());
    assert!(zones.len() > 500, "{} zone cells", zones.len());
    // The Field of Dreamers, where the mode's Val'sharah is played (`data::MAPS`).
    let field = [2330.0f32, 6480.0, 147.0];
    let dist = |p: [f32; 3]| {
        (0..3)
            .map(|i| (p[i] - field[i]).powi(2))
            .sum::<f32>()
            .sqrt()
    };
    // 1.12.1's blend lays the spheres covering a point farthest first, each over what is under
    // it by its alpha: a sphere's share is its alpha times what the nearer ones leave.
    let alpha = |l: &benilla_legion::lights::LegionLight| {
        let d = dist(l.pos);
        if d >= l.falloff_end {
            0.0
        } else if d <= l.falloff_start {
            1.0
        } else {
            (l.falloff_end - d) / (l.falloff_end - l.falloff_start)
        }
    };
    let mut covering: Vec<_> = lights
        .iter()
        .filter(|l| !l.global && alpha(l) > 0.0)
        .collect();
    covering.sort_by(|a, b| dist(a.pos).total_cmp(&dist(b.pos)));
    let mut left = 1.0;
    let mut shares = Vec::new();
    for l in &covering {
        shares.push((l.id, l.params[0], left * alpha(l)));
        left *= 1.0 - alpha(l);
    }
    println!(
        "shares, nearest first: {:?}; the global's {left}",
        &shares[..shares.len().min(4)]
    );
    // ZoneLight 1150, "ValSharah" (Light 5722, its clear weather's params 3064), lights it all.
    let valsharah: f32 = shares.iter().filter(|s| s.1 == 3064).map(|s| s.2).sum();
    assert!(valsharah > 0.99, "{valsharah}");
    // A spot still lights over its zone: halfway to its full strength's edge, it is laid last.
    let spots: Vec<_> = lights
        .iter()
        .filter(|l| !l.global && l.id < ZONE_LIGHT_BASE && l.falloff_start > 1.0)
        .collect();
    for s in &spots {
        let at = [s.pos[0] + s.falloff_start / 2.0, s.pos[1], s.pos[2]];
        let d = |l: &benilla_legion::lights::LegionLight| {
            (0..3)
                .map(|i| (l.pos[i] - at[i]).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        let last = lights
            .iter()
            .filter(|l| !l.global && d(l) <= l.falloff_end)
            .min_by(|a, b| d(a).total_cmp(&d(b)))
            .unwrap();
        assert!(
            last.id < ZONE_LIGHT_BASE,
            "spot {} under cell {}",
            s.id,
            last.id
        );
    }
    // Its noon is brighter than the Broken Isles' global, which lit the field before.
    let noon = |id: u32| {
        let p = params.iter().find(|p| p.id == id).unwrap();
        let c = p.int_bands[1]
            .iter()
            .rev()
            .find(|(t, _)| *t <= 1440)
            .map_or(0, |k| k.1);
        (c >> 16 & 0xff) + (c >> 8 & 0xff) + (c & 0xff)
    };
    let global = lights.iter().find(|l| l.global).unwrap().params[0];
    println!(
        "ambient at noon: Val'sharah {}, global {}",
        noon(3064),
        noon(global)
    );
    assert!(noon(3064) > noon(global));
}

#[test]
#[ignore = "needs a Legion install and WoWDBDefs' definitions: LEGION_URL, LEGION_CACHE, LEGION_DBD"]
fn an_hd_body_fits_1_12_1s_indices() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let dbd = PathBuf::from(std::env::var("LEGION_DBD").expect("LEGION_DBD"));
    let library = Library::open(source, Some(&dbd)).unwrap();
    let casc = casc();
    let race = library.race(10).unwrap();
    // The Blood Elf's HD bodies, male and female.
    for (sex, file) in [(0u8, 1100087u32), (1, 1100258)] {
        let mut styles: Vec<(u8, u32)> = race
            .hair
            .iter()
            .filter(|((s, _), _)| *s == sex)
            .map(|&((_, style), g)| (style, g))
            .collect();
        styles.sort();
        let hair: Vec<u32> = styles.iter().map(|s| s.1).collect();
        let kept = library.body_fit(file, &hair).unwrap();

        let chunks = md21::split(&casc.read_id(file).unwrap()).unwrap();
        let skel =
            benilla_legion::skel::parse(&casc.read_id(chunks.skel.unwrap()).unwrap()).unwrap();
        let md20 = benilla_legion::skel::merge(&chunks.md20, &skel).unwrap();
        let skin = casc.read_id(chunks.skins[0]).unwrap();
        let parsed = skin::parse(&skin).unwrap();
        let indices: u32 = parsed
            .sections
            .iter()
            .filter(|s| kept.contains(&s.id))
            .map(|s| u32::from(s.index_count))
            .sum();
        let styles_kept = hair
            .iter()
            .filter(|&&g| g == 0 || u16::try_from(g).is_ok_and(|g| kept.contains(&g)))
            .count();
        println!(
            "{file}: {} of {} indices, {styles_kept} of {} hair styles",
            indices,
            parsed.indices.len(),
            hair.len()
        );
        assert!(indices <= benilla_legion::fit::MAX_INDICES);
        assert!(styles_kept >= hair.len() / 3, "{styles_kept} styles");
        // The body, and every group 1.12.1 draws that the model has: hands, feet, ears, sleeves,
        // legs and the rest, every variant of each.
        assert!(kept.contains(&0));
        for g in benilla_legion::fit::GROUPS.iter().filter(|&&g| g != 0) {
            for s in parsed.sections.iter().filter(|s| s.id / 100 == *g) {
                assert!(kept.contains(&s.id), "{}", s.id);
            }
        }

        // It transcodes whole, and the client draws it from the skin's vertices alone.
        let vanilla = transcode::to_vanilla(
            &md20,
            &skin,
            &[],
            |id| kept.contains(&id),
            &Default::default(),
        )
        .unwrap();
        let m = benilla_m2::parse_m2(&mut Cursor::new(&vanilla[..])).unwrap();
        assert_eq!(m.model().vertices.len(), parsed.vertices.len());
        let draws = benilla_formats::parse_m2_render_submeshes(&vanilla, "", &[]).unwrap();
        assert!(draws.len() > 20, "{} draws", draws.len());
    }
}

#[test]
#[ignore = "needs a Legion install: LEGION_URL, LEGION_CACHE"]
fn the_field_of_dreamers_carries_legions_ground() {
    let source: AnySource = Box::new(Remote {
        url: std::env::var("LEGION_URL").expect("LEGION_URL"),
        cache: PathBuf::from(std::env::var("LEGION_CACHE").expect("LEGION_CACHE")),
    });
    let library = Library::open(source, None).unwrap();
    let adt = library
        .read("world/maps/legionbrokenisles/legionbrokenisles_19_27.adt")
        .unwrap()
        .unwrap();
    // The tile's chunks, magics as written (reversed).
    let mut chunks = Vec::new();
    let mut at = 0;
    while at + 8 <= adt.len() {
        let size = u32::from_le_bytes(adt[at + 4..at + 8].try_into().unwrap()) as usize;
        chunks.push((&adt[at..at + 4], &adt[at + 8..at + 8 + size]));
        at += 8 + size;
    }
    let find = |m: &[u8]| chunks.iter().find(|c| c.0 == m).unwrap().1;
    let split = |b: &[u8]| -> Vec<String> {
        let mut v: Vec<String> = b
            .split(|&c| c == 0)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        v.pop();
        v
    };
    let mtex = split(find(b"XETM"));
    let heights = split(find(b"XTHM"));
    let mtxp = find(b"PXTM");
    println!("{mtex:?}\n{heights:?}");
    // Ten ground textures, each by its specular variant, each with its height texture and scale.
    assert_eq!(mtex.len(), 10);
    assert_eq!(heights.len(), mtex.len());
    assert_eq!(mtxp.len(), mtex.len() * 16);
    assert!(mtex.iter().all(|t| t.starts_with("Legion\\spec\\")));
    assert!(heights.iter().all(|h| h.starts_with("Legion\\")));
    for t in mtex.iter().chain(&heights) {
        let path = t.replace('\\', "/").to_ascii_lowercase();
        let blp = library.read(&path).unwrap().unwrap();
        assert_eq!(&blp[..4], b"BLP2", "{t}");
        // The terrain loader's `_s` ask answers the same file.
        if let Some(stem) = path.strip_suffix(".blp").filter(|p| p.contains("/spec/")) {
            assert_eq!(
                library.read(&format!("{stem}_s.blp")).unwrap().unwrap(),
                blp
            );
        }
    }
    // Every chunk's vertex colours where the header's `+0x74` points.
    let mut coloured = 0;
    for (_, mcnk) in chunks.iter().filter(|c| c.0 == b"KNCM") {
        let ofs = u32::from_le_bytes(mcnk[0x74..0x78].try_into().unwrap()) as usize;
        if ofs != 0 && &mcnk[ofs - 8..ofs - 4] == b"VCCM" {
            coloured += 1;
        }
    }
    assert_eq!(coloured, 256);
    // And 1.12.1's reader still reads the tile whole.
    let tile = benilla_formats::adt_to_tile_mesh(&adt).unwrap();
    assert_eq!(tile.chunks.len(), 256);
}
