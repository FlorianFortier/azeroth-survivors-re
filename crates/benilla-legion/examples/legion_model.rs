//! Read one Legion model from a remote Legion install and draw it: its MD21 chunks, its MD20
//! model, its first skin profile and its textures, written to `<out>` as an `.obj` with its
//! textures, and drawn in its bind pose by a small software rasterizer to `<out>/preview.png`.
//! It also writes the model transcoded to 1.12.1 (`vanilla.m2`), as the client's loader reads it.
//! No GPU needed; nothing it writes belongs in git.
//!
//! ```text
//! LEGION_URL=https://…/WoW_Legion_7.3.5.26972 LEGION_CACHE=/tmp/legion \
//! LEGION_LISTFILE=/tmp/community-listfile.csv LEGION_DBD=/tmp/WoWDBDefs/definitions \
//!   cargo run --release -p benilla-legion --example legion_model -- creature/illidan2/illidan2.m2 /tmp/out
//! ```
//!
//! With `LEGION_DBD` (a folder of WoWDBDefs' `.dbd` definitions), the model's creature display
//! (`CreatureDisplayInfo`, the first of the model's, or `LEGION_DISPLAY`) gives the textures it
//! leaves to its display (types 11 to 13) and the geosets shown. Without it, they are guessed: the
//! `.blp` files beside the model in the listfile, and each geoset group's first variant.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use benilla_casc::remote::Remote;
use benilla_casc::Casc;
use benilla_legion::db2::{Raw, Table};
use benilla_legion::dbd::Definition;
use benilla_legion::display::{self, CreatureDisplay};
use benilla_legion::{md21, model, skin, transcode};

type Res<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Image {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Image {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = ((u.rem_euclid(1.0)) * self.w as f32) as u32 % self.w;
        let y = ((v.rem_euclid(1.0)) * self.h as f32) as u32 % self.h;
        let i = ((y * self.w + x) * 4) as usize;
        let p = &self.rgba[i..i + 4];
        [p[0], p[1], p[2], p[3]].map(|c| f32::from(c) / 255.0)
    }
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (target, out) = match args.as_slice() {
        [t, o] => (t.clone(), PathBuf::from(o)),
        _ => return Err("usage: legion_model <path | file data id> <out dir>".into()),
    };
    std::fs::create_dir_all(&out)?;
    let casc = Casc::open(Remote {
        url: std::env::var("LEGION_URL")?,
        cache: PathBuf::from(std::env::var("LEGION_CACHE")?),
    })?;
    let listfile = std::env::var("LEGION_LISTFILE").ok().map(read_listfile);
    let (id, file) = match target.parse::<u32>() {
        Ok(id) => (id, casc.read_id(id)?),
        Err(_) => (casc.id_of(&target).unwrap_or(0), casc.read_path(&target)?),
    };
    let path = listfile
        .as_ref()
        .and_then(|l| l.by_id.get(&id).cloned())
        .unwrap_or(target.clone());

    let chunks = md21::split(&file)?;
    let m = model::parse(&chunks.md20)?;
    println!(
        "{path} (id {id}): MD20 v{}, \"{}\", {} vertices, {} bones, {} sequences ({} in .anim files), {} skin profiles, {} textures {:?}, other chunks {:?}",
        m.version,
        m.name,
        m.vertices.len(),
        m.bones.len(),
        m.sequences,
        chunks.anims.len(),
        m.skin_profiles,
        m.textures.len(),
        m.textures.iter().map(|t| t.kind).collect::<Vec<_>>(),
        chunks.other.iter().map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>(),
    );
    let skin_id = *chunks.skins.first().ok_or("no SFID skin")?;
    let s = skin::parse(&casc.read_id(skin_id)?)?;
    println!(
        "skin {skin_id}: {} vertices, {} triangles, {} sections {:?}, {} batches",
        s.vertices.len(),
        s.indices.len() / 3,
        s.sections.len(),
        s.sections.iter().map(|x| x.id).collect::<Vec<_>>(),
        s.batches.len()
    );

    // The creature display, from the DB2 tables, if the definitions are at hand.
    let shown_display: Option<CreatureDisplay> = match std::env::var("LEGION_DBD") {
        Ok(dir) => {
            let info = table(&casc, &dir, "CreatureDisplayInfo")?;
            let models = table(&casc, &dir, "CreatureModelData")?;
            let all = display::displays_of_model(&info, &models, id);
            println!(
                "{} creature displays of this model: {:?}",
                all.len(),
                all.iter().map(|d| d.id).take(12).collect::<Vec<_>>()
            );
            let wanted = std::env::var("LEGION_DISPLAY")
                .ok()
                .and_then(|d| d.parse::<u32>().ok());
            let d = match wanted {
                Some(w) => display::creature_display(&info, &models, w),
                None => all.into_iter().next(),
            };
            if let Some(d) = &d {
                println!("display {d:?}");
            }
            d
        }
        Err(_) => None,
    };

    // Textures: the model's own by TXID, a creature's replaceable ones guessed from the listfile.
    let folder = path.rsplit_once('/').map_or("", |(f, _)| f).to_string();
    let mut guesses: Vec<u32> = listfile
        .as_ref()
        .map(|l| l.blps_in(&folder))
        .unwrap_or_default();
    // The one named as the model first, then the `skin` ones.
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".m2")
        .to_string();
    guesses.sort_by_key(|g| {
        let name = listfile
            .as_ref()
            .and_then(|l| l.by_id.get(g))
            .cloned()
            .unwrap_or_default();
        let own = name.ends_with(&format!("/{stem}.blp"));
        (!own, !name.contains("skin"), name)
    });
    let mut images: HashMap<usize, Image> = HashMap::new();
    for (i, t) in m.textures.iter().enumerate() {
        // `TXID`'s file, else the path the record names.
        let named = chunks
            .textures
            .get(i)
            .copied()
            .filter(|&f| f != 0)
            .or_else(|| {
                (!t.filename.is_empty())
                    .then(|| casc.id_of(&t.filename))
                    .flatten()
            });
        if !t.filename.is_empty() {
            println!("texture {i}: names {}", t.filename);
        }
        let from_display = shown_display.as_ref().and_then(|d| d.texture_for(t.kind));
        let fid = named.or(from_display).or_else(|| match t.kind {
            11..=13 => guesses
                .get((t.kind - 11) as usize)
                .or(guesses.first())
                .copied(),
            _ => None,
        });
        let Some(fid) = fid else {
            println!("texture {i}: type {}, none", t.kind);
            continue;
        };
        match casc
            .read_id(fid)
            .map_err(|e| e.to_string())
            .and_then(|b| benilla_blp::decode(&b).map_err(|e| e.to_string()))
        {
            Ok(blp) => {
                println!(
                    "texture {i}: type {}, file {fid} {}, {}x{}",
                    t.kind,
                    listfile
                        .as_ref()
                        .and_then(|l| l.by_id.get(&fid))
                        .map_or("", |s| s.as_str()),
                    blp.width,
                    blp.height
                );
                let level = &blp.mips[0];
                image::save_buffer(
                    out.join(format!("tex{i}.png")),
                    &level.rgba,
                    level.width,
                    level.height,
                    image::ExtendedColorType::Rgba8,
                )?;
                images.insert(
                    i,
                    Image {
                        w: level.width,
                        h: level.height,
                        rgba: level.rgba.clone(),
                    },
                );
            }
            Err(e) => println!("texture {i}: file {fid}: {e}"),
        }
    }

    // The sections the display shows; without one, the body and each group's first variant.
    let shown = |id: u16| match &shown_display {
        Some(d) => d.shows(id),
        None => id == 0 || id % 100 == 1,
    };
    write_obj(&out, &m, &s, &shown)?;

    // The model as 1.12.1 reads it, its textures by their virtual paths.
    let names: Vec<Option<String>> =
        transcode::texture_files(&m, &chunks.textures, shown_display.as_ref(), |p| {
            casc.id_of(p)
        })
        .into_iter()
        .map(|f| f.map(transcode::virtual_path))
        .collect();
    let files: Vec<(u16, u16, Vec<u8>)> = chunks
        .anims
        .iter()
        .filter_map(|&(a, v, f)| Some((a, v, casc.read_id(f).ok()?)))
        .collect();
    let animation = transcode::Animation {
        files: &files,
        fallbacks: &[],
    };
    let vanilla = transcode::to_vanilla(
        &chunks.md20,
        &casc.read_id(skin_id)?,
        &names,
        shown,
        &animation,
    )?;
    std::fs::write(out.join("vanilla.m2"), &vanilla)?;
    let draws = benilla_formats::parse_m2_render_submeshes(&vanilla, "", &[])?;
    println!(
        "vanilla.m2: {} bytes (the Legion model {}, its .anim files {}), {} draws, textures {:?}",
        vanilla.len(),
        chunks.md20.len(),
        files.iter().map(|f| f.2.len()).sum::<usize>(),
        draws.len(),
        names
    );
    let picture = render(&m, &s, &images, &shown, 900);
    image::save_buffer(
        out.join("preview.png"),
        &picture,
        900,
        900,
        image::ExtendedColorType::Rgba8,
    )?;
    println!("wrote {}", out.display());
    Ok(())
}

/// A DB2 table from the install, typed by its definition in `dir`.
fn table(casc: &Casc<Remote>, dir: &str, name: &str) -> Res<Table> {
    let raw = Raw::parse(&casc.read_path(&format!("DBFilesClient/{name}.db2"))?)?;
    let def = Definition::parse(&std::fs::read_to_string(
        Path::new(dir).join(format!("{name}.dbd")),
    )?)?;
    let layout = def
        .layout(raw.layout_hash, "7.3.5.26972")
        .ok_or_else(|| format!("{name}: no layout {:08X}", raw.layout_hash))?;
    Ok(Table::new(&raw, layout))
}

struct Listfile {
    by_id: HashMap<u32, String>,
}

impl Listfile {
    fn blps_in(&self, folder: &str) -> Vec<u32> {
        self.by_id
            .iter()
            .filter(|(_, p)| {
                p.ends_with(".blp") && p.rsplit_once('/').is_some_and(|(f, _)| f == folder)
            })
            .map(|(&id, _)| id)
            .collect()
    }
}

fn read_listfile(path: String) -> Listfile {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    Listfile {
        by_id: text
            .lines()
            .filter_map(|l| {
                let (id, p) = l.trim_end().split_once(';')?;
                Some((id.parse().ok()?, p.to_string()))
            })
            .collect(),
    }
}

fn write_obj(out: &Path, m: &model::Model, s: &skin::Skin, shown: &dyn Fn(u16) -> bool) -> Res<()> {
    let mut obj = String::from("# a Legion model's bind pose, its first skin profile\n");
    for v in &m.vertices {
        let p = v.position;
        writeln!(obj, "v {} {} {}", p[0], p[2], -p[1])?;
        writeln!(obj, "vt {} {}", v.uv[0], 1.0 - v.uv[1])?;
    }
    for sec in s.sections.iter().filter(|x| shown(x.id)) {
        writeln!(obj, "g section{}", sec.id)?;
        for t in s.triangles(sec) {
            let [a, b, c] = t.map(|i| i + 1);
            writeln!(obj, "f {a}/{a} {b}/{b} {c}/{c}")?;
        }
    }
    std::fs::write(out.join("model.obj"), obj)?;
    Ok(())
}

/// A small rasterizer: a three-quarter view fit to the model, opaque and alpha-keyed batches with
/// a depth buffer and a lambert light, then blended and additive ones over them.
fn render(
    m: &model::Model,
    s: &skin::Skin,
    images: &HashMap<usize, Image>,
    shown: &dyn Fn(u16) -> bool,
    size: u32,
) -> Vec<u8> {
    let n = (size * size) as usize;
    let mut color = vec![[0.0f32; 3]; n];
    for (i, c) in color.iter_mut().enumerate() {
        let y = (i as u32 / size) as f32 / size as f32;
        *c = [0.16 + 0.1 * y, 0.17 + 0.1 * y, 0.2 + 0.12 * y];
    }
    let mut depth = vec![f32::INFINITY; n];
    // Model space: x forward, y left, z up. The camera looks at the model's front, turned 35°.
    let (sa, ca) = 35f32.to_radians().sin_cos();
    let view = |p: [f32; 3]| -> [f32; 3] {
        let x = p[0] * ca - p[1] * sa;
        let y = p[0] * sa + p[1] * ca;
        [-y, p[2], -x] // screen right, up, depth (smaller is nearer)
    };
    let pts: Vec<[f32; 3]> = m.vertices.iter().map(|v| view(v.position)).collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &pts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let scale = 0.9 * size as f32 / (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-3);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let screen = |p: [f32; 3]| -> [f32; 3] {
        [
            size as f32 / 2.0 + (p[0] - mid[0]) * scale,
            size as f32 / 2.0 - (p[1] - mid[1]) * scale,
            p[2],
        ]
    };
    let light = {
        let l = view([0.6, -0.5, 0.7]);
        let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / len, l[1] / len, -l[2] / len]
    };
    let mut batches: Vec<&skin::Batch> = s
        .batches
        .iter()
        .filter(|b| {
            s.sections
                .get(b.section as usize)
                .is_some_and(|x| shown(x.id))
        })
        .collect();
    let blend_of = |b: &skin::Batch| m.materials.get(b.material as usize).map_or(0, |x| x.blend);
    batches.sort_by_key(|b| (blend_of(b) >= 2, b.priority));
    for b in batches {
        let blend = blend_of(b);
        let tex = m.texture_of(b.texture_combo).and_then(|t| images.get(&t));
        let sec = &s.sections[b.section as usize];
        for tri in s.triangles(sec) {
            let v = tri.map(|i| screen(pts[i]));
            let area = (v[1][0] - v[0][0]) * (v[2][1] - v[0][1])
                - (v[2][0] - v[0][0]) * (v[1][1] - v[0][1]);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = v.iter().map(|p| p[0]).fold(f32::MAX, f32::min).max(0.0) as u32;
            let x1 = (v.iter().map(|p| p[0]).fold(f32::MIN, f32::max) as u32).min(size - 1);
            let y0 = v.iter().map(|p| p[1]).fold(f32::MAX, f32::min).max(0.0) as u32;
            let y1 = (v.iter().map(|p| p[1]).fold(f32::MIN, f32::max) as u32).min(size - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 =
                        ((v[1][0] - px) * (v[2][1] - py) - (v[2][0] - px) * (v[1][1] - py)) / area;
                    let w1 =
                        ((v[2][0] - px) * (v[0][1] - py) - (v[0][0] - px) * (v[2][1] - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * v[0][2] + w1 * v[1][2] + w2 * v[2][2];
                    let i = (y * size + x) as usize;
                    if z >= depth[i] {
                        continue;
                    }
                    let vs = tri.map(|k| &m.vertices[k]);
                    let uv = [
                        w0 * vs[0].uv[0] + w1 * vs[1].uv[0] + w2 * vs[2].uv[0],
                        w0 * vs[0].uv[1] + w1 * vs[1].uv[1] + w2 * vs[2].uv[1],
                    ];
                    let texel = tex.map_or([0.7, 0.7, 0.7, 1.0], |t| t.sample(uv[0], uv[1]));
                    if blend == 1 && texel[3] < 0.5 {
                        continue;
                    }
                    let nrm = {
                        let nn = [0, 1, 2].map(|k| {
                            w0 * vs[0].normal[k] + w1 * vs[1].normal[k] + w2 * vs[2].normal[k]
                        });
                        let nv = view(nn);
                        let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2])
                            .sqrt()
                            .max(1e-6);
                        [nv[0] / len, nv[1] / len, -nv[2] / len]
                    };
                    let lit = 0.45
                        + 0.75
                            * (nrm[0] * light[0] + nrm[1] * light[1] + nrm[2] * light[2]).max(0.0);
                    let rgb = [0, 1, 2].map(|k| texel[k] * lit);
                    match blend {
                        0 | 1 => {
                            color[i] = rgb;
                            depth[i] = z;
                        }
                        2 => {
                            let a = texel[3];
                            color[i] = [0, 1, 2].map(|k| color[i][k] * (1.0 - a) + rgb[k] * a);
                        }
                        _ => {
                            color[i] = [0, 1, 2].map(|k| color[i][k] + texel[k] * texel[3]);
                        }
                    }
                }
            }
        }
    }
    color
        .iter()
        .flat_map(|c| {
            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
            [q(c[0]), q(c[1]), q(c[2]), 255]
        })
        .collect()
}
