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
    let vanilla = transcode::to_vanilla(&chunks.md20, &skin_file, &names, shows).unwrap();

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

    // The client's own loader builds its draw from it: two batches, not the glow over the
    // body, whose colour is transparent in every sequence but two, and so at its first key.
    let draws = benilla_formats::parse_m2_render_submeshes(&vanilla, "", &[]).unwrap();
    println!("{} render submeshes", draws.len());
    assert_eq!((kept, draws.len()), (3, 2));
    assert!(draws.iter().all(|d| !d.indices.is_empty()));
}
