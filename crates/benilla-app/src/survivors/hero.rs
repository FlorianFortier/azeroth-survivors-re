//! The hero: its body, WASD movement over the terrain, the top-down camera, and its health.

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::outfit;
use super::units::{self, flat, wow_to_bevy, yaw_toward, Fx, Marks, UnitSpec};
use super::{controls_active, HeroState, LocalHero, Phase, Run, Selection, Session, Vitals};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

#[derive(Component, Default)]
pub(super) struct Hero {
    /// Vertical velocity in yards/second while airborne; 0 on the ground.
    pub(super) vy: f32,
    /// Height above the ground while airborne. Kept here, not in the transform: the creature
    /// ground clamp (`net::motion::ground_clamp_creatures`) re-seats every unit on the floor at
    /// the top of the frame, so the arc is re-applied over the ground each frame.
    pub(super) air: f32,
    /// The ground-plane velocity carried through the air: WoW locks a jump to the momentum it
    /// took off with, so the keys no longer steer once the feet leave the ground.
    pub(super) drift: Vec3,
    /// The arc's direction is spent: set by a running take-off, or by the one nudge a standing
    /// jump may take (`player::mover::step`'s `arc_dirs_set`).
    pub(super) arc_set: bool,
}

/// The body radius enemies stop at.
pub(super) const HERO_RADIUS: f32 = 0.7;

/// The hop's initial upward speed and the gravity that brings it back down — the reference
/// `CMovement` jump seed and gravity (`player/state.rs`'s `JUMP_SPEED` / `GRAVITY`).
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;
/// Backpedal over run, the stock 4.5 / 7.0 (`player/state.rs`'s `RUN_BACK_RATIO`): any run with a
/// rearward part, backpedal and its diagonals, goes at the backward speed; strafing is full speed.
const RUN_BACK_RATIO: f32 = 4.5 / 7.0;
/// Walk over run, the stock 2.5 / 7.0: a jump taken standing may be nudged once, at walk speed
/// (`player/state.rs`'s `AIR_NUDGE_SPEED`, the live `min(walk, run)`).
const WALK_RATIO: f32 = 2.5 / 7.0;

/// Radians of orbit per raw mouse unit while right-dragging.
const ORBIT_RATE: f32 = 0.008;
/// Radians of turn per raw mouse unit in the chase view, before `WOW_MOUSE_SENS`.
const CHASE_RATE: f32 = 0.0028;
/// The chase view's elevation range: from a little under the shoulders looking up, to high above.
const CHASE_PITCH_MIN: f32 = -0.35;
const CHASE_PITCH_MAX: f32 = 1.25;
const CHASE_PITCH: f32 = 0.32;
/// The chase boom: its rest length, its zoom range, and the point it hangs from above the feet.
const CHASE_DIST: f32 = 9.0;
const CHASE_ZOOM_MIN: f32 = 3.0;
const CHASE_ZOOM_MAX: f32 = 24.0;
const CHASE_HEIGHT: f32 = 1.9;
/// The top-down view's rest distance.
const FIXED_DIST: f32 = 30.0;
/// Elevation clamp (radians): ~10°..~83° above the horizon, so the camera never dives underground.
const PITCH_MIN: f32 = 0.18;
const PITCH_MAX: f32 = 1.45;
/// The locked view's tilt.
const FIXED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

/// The view; `C` cycles them.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum CamMode {
    /// Third person behind the hero, the mouse turning the view and the hero with it, WASD
    /// relative to where the camera looks: the 3D survivors feel.
    #[default]
    Chase,
    /// Locked top-down (61°), laptop-friendly: the hero faces the cursor.
    Fixed,
    /// Unlocked top-down: a mouse drag swings and tilts it around the hero.
    Orbit,
}

impl CamMode {
    fn next(self) -> Self {
        match self {
            CamMode::Chase => CamMode::Fixed,
            CamMode::Fixed => CamMode::Orbit,
            CamMode::Orbit => CamMode::Chase,
        }
    }
}

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
    pub orbit: f32,
    /// Orbit azimuth (radians) around the hero; right-drag swings it during play.
    pub yaw: f32,
    /// Elevation above the horizon (radians), clamped so the camera never dives underground.
    pub pitch: f32,
    pub mode: CamMode,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            dist: CHASE_DIST,
            orbit: 0.6,
            yaw: 0.0,
            pitch: CHASE_PITCH,
            mode: CamMode::Chase,
        }
    }
}

impl CamRig {
    /// Where the camera looks along the ground: the chase view's forward.
    pub(super) fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-sy, 0.0, -cy)
    }

    /// WASD's `dir` (x right, z back) turned to the camera's heading.
    fn relative(&self, dir: Vec3) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(dir.x * cy + dir.z * sy, 0.0, dir.z * cy - dir.x * sy)
    }

    /// Switch to `mode` at its rest framing; the chase keeps the heading, so the view does not spin.
    fn switch(&mut self, mode: CamMode) {
        self.mode = mode;
        match mode {
            CamMode::Chase => {
                self.pitch = CHASE_PITCH;
                self.dist = CHASE_DIST;
            }
            CamMode::Fixed => {
                self.yaw = 0.0;
                self.pitch = FIXED_PITCH;
                self.dist = FIXED_DIST;
            }
            CamMode::Orbit => {
                self.pitch = self.pitch.clamp(PITCH_MIN, PITCH_MAX);
                self.dist = FIXED_DIST;
            }
        }
    }
}

/// The chase view holds the mouse: the cursor is hidden and kept in the window while a run is
/// played, and given back for menus, cards, the pause screen, an unfocused window, or while Alt is
/// held (to reach the minimap's buttons).
pub(super) fn capture_cursor(
    phase: Res<State<Phase>>,
    run: Res<Run>,
    rig: Res<CamRig>,
    keys: Res<ButtonInput<KeyCode>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut windows: Query<
        (&mut Window, &mut bevy::window::CursorOptions),
        With<bevy::window::PrimaryWindow>,
    >,
    mut held: Local<bool>,
) {
    let Ok((mut window, mut cursor)) = windows.single_mut() else {
        return;
    };
    let want = rig.mode == CamMode::Chase
        && *phase.get() == Phase::Playing
        && run.begun
        && window.focused
        && !loading.covering()
        && !keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if want == *held {
        return;
    }
    *held = want;
    // Written only on a change: a `Mut` every frame makes the windowing backend re-apply it.
    if want {
        // Windows confines rather than locks; raw mouse motion still turns the view at the edge.
        cursor.grab_mode = if cfg!(windows) {
            bevy::window::CursorGrabMode::Confined
        } else {
            bevy::window::CursorGrabMode::Locked
        };
        cursor.visible = false;
    } else {
        cursor.grab_mode = bevy::window::CursorGrabMode::None;
        cursor.visible = true;
        // Given back at the centre, where the crosshair was.
        let center = Vec2::new(window.width(), window.height()) / 2.0;
        window.set_cursor_position(Some(center));
    }
}

/// The chase view's crosshair, at the centre of the screen.
#[derive(Component)]
pub(super) struct Crosshair;

pub(super) fn setup_crosshair(mut commands: Commands) {
    commands
        .spawn((
            Crosshair,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(390),
            Visibility::Hidden,
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.9, 0.6, 0.9)),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                Pickable::IGNORE,
            ));
        });
}

pub(super) fn show_crosshair(
    phase: Res<State<Phase>>,
    rig: Res<CamRig>,
    run: Res<Run>,
    mut crosshair: Query<(&mut Visibility, &mut Node), With<Crosshair>>,
) {
    let on = rig.mode == CamMode::Chase
        && matches!(phase.get(), Phase::Playing | Phase::LevelUp)
        && run.begun
        && run.dead_t.is_none();
    for (mut v, mut n) in &mut crosshair {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        let display = if on { Display::Flex } else { Display::None };
        if n.display != display {
            n.display = display;
        }
    }
}

/// The mouse-turn multiplier, `WOW_MOUSE_SENS` (`?mouse_sens=` on the web), 1 by default.
fn mouse_sens() -> f32 {
    static SENS: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SENS.get_or_init(|| {
        crate::webenv::var("WOW_MOUSE_SENS")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .map_or(1.0, |v| v.clamp(0.1, 10.0))
    })
}

/// Where a battleground's runs start.
pub(super) fn arena(map: &data::MapDef) -> Vec3 {
    wow_to_bevy(map.center)
}

/// The world is the battleground being stood in ([`Selection::stage`]): entering one on another
/// continent worldports, which raises the loading screen until the new ground is in.
pub(super) fn follow_map(
    selection: Res<Selection>,
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
) {
    let Some(mut current) = current else {
        return;
    };
    let want = selection.stage_def().map;
    if current.0 != want {
        current.0 = want;
    }
}

/// The hero, weapons drawn: the race's own body in what it wears, or the class trainer's look
/// ([`outfit::Body`]).
pub(super) fn spawn_hero(
    commands: &mut Commands,
    class: &'static ClassDef,
    body: &outfit::Body,
    race: u8,
    sex: u8,
    serial: u64,
    pos: Vec3,
) -> Entity {
    let bytes0 = u32::from(race)
        | u32::from(class.id) << 8
        | u32::from(sex) << 16
        | u32::from(class.power) << 24;
    let display = match body {
        outfit::Body::Player { display, .. } | outfit::Body::Trainer { display } => *display,
    };
    let spec = UnitSpec {
        serial,
        display,
        scale: 1.0,
        level: 60,
        faction: units::FACTION_FRIENDLY,
        bytes0,
        weapons: class.weapons,
        sheath: class.sheath,
    };
    let e = match body {
        outfit::Body::Player {
            appearance, worn, ..
        } => {
            let mut extra: Vec<(u16, u32)> = outfit::appearance_fields(appearance).to_vec();
            extra.extend(
                worn.iter()
                    .map(|w| (outfit::visible_field(w.slot), outfit::entry(w))),
            );
            outfit::queue_templates(commands, worn.iter().map(outfit::entry).collect());
            units::spawn_player(commands, &spec, &extra, pos, 0.0)
        }
        outfit::Body::Trainer { .. } => units::spawn_unit(commands, &spec, pos, 0.0),
    };
    commands.entity(e).insert((Hero::default(), Marks::buffs()));
    e
}

/// Midday over the battleground, whatever the clock says.
pub(super) const MINUTE_OF_DAY: u32 = 780;

pub(super) fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != MINUTE_OF_DAY {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = MINUTE_OF_DAY;
    }
}

/// The ground under the mouse cursor, `None` with the cursor off the window. The locked camera's
/// hero faces it, and the aimed mobility spells (Blink, Charge) go toward it.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub(super) Option<Vec3>);

/// Cast the cursor onto the terrain: the camera ray meets the plane at the hero's height, then the
/// terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<LocalHero>, Without<WorldCamera>)>,
    rig: Res<CamRig>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let point = (|| {
        let (cam, tf) = cams.single().ok()?;
        // The chase view aims where the crosshair is; the others where the pointer is.
        let cursor = if rig.mode == CamMode::Chase {
            cam.logical_viewport_size()? / 2.0
        } else {
            windows.single().ok()?.cursor_position()?
        };
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    })();
    if aim.0 != point {
        aim.0 = point;
    }
}

/// WASD over the terrain within the battleground's leash, and the streaming focus on the hero.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    phase: Res<State<Phase>>,
    session: Res<Session>,
    run: Res<Run>,
    selection: Res<Selection>,
    rig: Res<CamRig>,
    aim: Res<AimPoint>,
    world: WorldPoint,
    auto: Option<Res<super::auto::Auto>>,
    mut player: ResMut<crate::player::Player>,
    mut hero: Query<(&mut Transform, &mut MovementState, &mut Hero), With<LocalHero>>,
) {
    // Steering, on a hero still standing and before the run's last word.
    let steering = controls_active(*phase.get(), &session)
        && run.begun
        && run.dead_t.is_none()
        && !run.won
        && !run.lost;
    let focus = match hero.single_mut() {
        Ok((mut tf, mut mv, mut hm)) => {
            let dt = time.delta_secs();
            let mut dir = Vec3::ZERO;
            if let Some(auto) = auto.as_deref().filter(|_| steering) {
                dir = super::auto::steer(auto);
            } else if steering {
                if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
                    dir.z -= 1.0;
                }
                if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
                    dir.z += 1.0;
                }
                if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
                    dir.x -= 1.0;
                }
                if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
                    dir.x += 1.0;
                }
                // Both mouse buttons held run forward, as in WoW: where the camera looks, or
                // toward the cursor from above.
                let both = mouse.pressed(MouseButton::Left) && mouse.pressed(MouseButton::Right);
                if both && rig.mode != CamMode::Fixed && dir.z >= 0.0 {
                    dir.z = -1.0;
                }
                // Chase and orbit move relative to the camera; the top-down view, to the world.
                if rig.mode != CamMode::Fixed {
                    dir = rig.relative(dir);
                } else if both && dir == Vec3::ZERO {
                    if let Some(to) = aim.0 {
                        let d = flat(to - tf.translation);
                        if d.length() > 0.5 {
                            dir = d.normalize();
                        }
                    }
                }
            }
            let mut pos = tf.translation;
            // The chase hero faces where the camera looks, and the top-down one the cursor, so
            // WASD backpedals and strafes as a player does; orbiting (or driven), he faces where he
            // runs.
            let face = match rig.mode {
                _ if auto.is_some() || !steering => None,
                CamMode::Chase => Some(rig.forward()),
                CamMode::Fixed => aim
                    .0
                    .map(|a| flat(a - pos))
                    .filter(|d| d.length() > 0.5)
                    .map(|d| d.normalize()),
                CamMode::Orbit => None,
            };
            if let Some(look) = face {
                let target = Quat::from_rotation_y(yaw_toward(look));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
            }
            let airborne = hm.vy != 0.0 || hm.air > 0.0;
            let run_speed = run.stats().speed * run.haste_mult();
            if airborne && steering {
                // In the air the arc owns the ground-plane motion: a running take-off keeps its
                // momentum, and a standing one may be nudged once, at walk speed.
                if dir != Vec3::ZERO && !hm.arc_set {
                    let dir = dir.normalize();
                    hm.drift = dir * run_speed * WALK_RATIO;
                    hm.arc_set = true;
                    mv.speed = run_speed * WALK_RATIO;
                    mv.flags = face.map_or(move_flags::FORWARD, |look| gait(look, dir));
                }
                pos += hm.drift * dt;
            } else if dir != Vec3::ZERO && dt > 0.0 {
                let dir = dir.normalize();
                let speed = run_speed * pace(face, dir);
                hm.drift = dir * speed;
                pos += hm.drift * dt;
                mv.speed = speed;
                mv.flags = match face {
                    Some(look) => gait(look, dir),
                    None => {
                        let target = Quat::from_rotation_y(yaw_toward(dir));
                        tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                        move_flags::FORWARD
                    }
                };
            } else {
                // WoW has no ground inertia: the body stops dead the frame the keys let go.
                hm.drift = Vec3::ZERO;
                if mv.speed != 0.0 || mv.flags != 0 {
                    mv.speed = 0.0;
                    mv.flags = 0;
                }
            }
            let map = selection.stage_def();
            let center = arena(map);
            let off = flat(pos - center);
            if off.length() > map.leash * 2.0 {
                // Just entered another battleground: start at its centre.
                pos = center;
            } else if off.length() > map.leash {
                let held = center + off.normalize() * map.leash;
                pos.x = held.x;
                pos.z = held.z;
            }
            let ground = world.terrain_height_under(pos).unwrap_or(pos.y);
            let playing = steering;
            // Space hops: lift off the terrain, then gravity brings the hero back down.
            if playing && keys.just_pressed(KeyCode::Space) && hm.vy == 0.0 && hm.air == 0.0 {
                hm.vy = JUMP_SPEED;
                // A moving jump is locked to its momentum; a standing one may steer once.
                hm.arc_set = hm.drift != Vec3::ZERO;
            }
            if playing && (hm.vy != 0.0 || hm.air > 0.0) {
                hm.vy -= GRAVITY * dt;
                hm.air += hm.vy * dt;
                if hm.air <= 0.0 {
                    hm.air = 0.0;
                    hm.vy = 0.0;
                    hm.arc_set = false;
                }
            } else {
                hm.air = 0.0;
                hm.vy = 0.0;
                hm.arc_set = false;
            }
            pos.y = ground + hm.air;
            // Feed the animation driver: `vertical_speed`'s sign and the `FALLING` flag are what
            // select the Jump (37/38) / Fall (40) animation (`creature_anim::select`).
            mv.vertical_speed = hm.vy;
            if hm.vy != 0.0 {
                mv.flags |= move_flags::FALLING;
            }
            if tf.translation != pos {
                tf.translation = pos;
            }
            pos
        }
        Err(_) => {
            let mut p = arena(selection.stage_def());
            if let Some(h) = world.terrain_height_under(p) {
                p.y = h;
            }
            p
        }
    };
    // The terrain streams and the loading screen waits on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// The view's controls, before the hero moves: `C` cycles the views, the wheel zooms, and the
/// mouse turns the chase view (while the cursor is held) or drags the orbit.
pub(super) fn turn_camera(
    phase: Res<State<Phase>>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<super::kit::KitFocus>,
    windows: Query<&bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    // On the lobby screen, or in a name field, C is a letter.
    if *phase.get() != Phase::Lobby && !focus.typing() && keys.just_pressed(KeyCode::KeyC) {
        let next = rig.mode.next();
        rig.switch(next);
    }
    // Multiplicative, so a notch moves as far by eye close in as far out.
    let (zmin, zmax) = if rig.mode == CamMode::Chase {
        (CHASE_ZOOM_MIN, CHASE_ZOOM_MAX)
    } else {
        (ZOOM_MIN, ZOOM_MAX)
    };
    // At the menu the wheel scrolls its lists; the camera's zoom is for the run.
    if matches!(phase.get(), Phase::Menu | Phase::Lobby) {
        wheel.clear();
    }
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(zmin, zmax);
    }
    if matches!(phase.get(), Phase::Menu | Phase::Lobby) {
        return;
    }
    match rig.mode {
        CamMode::Chase => {
            let held = windows
                .single()
                .is_ok_and(|c| !c.visible && c.grab_mode != bevy::window::CursorGrabMode::None);
            if held && *phase.get() == Phase::Playing {
                let rate = CHASE_RATE * mouse_sens();
                rig.yaw -= motion.delta.x * rate;
                rig.pitch =
                    (rig.pitch + motion.delta.y * rate).clamp(CHASE_PITCH_MIN, CHASE_PITCH_MAX);
            }
        }
        // Either mouse button drags the camera around the hero, as in WoW.
        CamMode::Orbit => {
            if mouse.any_pressed([MouseButton::Left, MouseButton::Right]) {
                rig.yaw -= motion.delta.x * ORBIT_RATE;
                rig.pitch = (rig.pitch + motion.delta.y * ORBIT_RATE).clamp(PITCH_MIN, PITCH_MAX);
            }
        }
        CamMode::Fixed => {}
    }
}

/// Where the menu shows the hero: the frame on screen the camera puts it in, in physical pixels
/// (centre and size), and whether it turns slowly on its own. Written by the main screen each
/// frame; with no frame, the menu's camera orbits the hero at the middle of the screen.
#[derive(Resource, Default)]
pub(super) struct MenuStage {
    pub frame: Option<(Vec2, Vec2)>,
    pub turning: bool,
}

/// The share of a stage's height the hero stands in, and the height that share is taken of (yd).
const STAGE_FILL: f32 = 0.8;
const STAGE_HERO_HEIGHT: f32 = 2.1;
/// The menu camera's elevation over the hero's middle (radians).
const STAGE_PITCH: f32 = 0.12;

/// The menu camera framing the hero into `frame` (centre and size, physical pixels) of a
/// `window`-sized view: far enough back that the hero fills [`STAGE_FILL`] of the frame's height,
/// then slid in the view's plane so the hero lands on the frame's centre rather than the screen's.
/// Returns the eye and the point it looks at.
fn stage_eye(focus: Vec3, orbit: f32, frame: (Vec2, Vec2), window: Vec2) -> (Vec3, Vec3) {
    let (center, size) = frame;
    let half_tan = (benilla_world::view::CAM_FOVY * 0.5).tan();
    // Pixels per yard at the hero's distance.
    let ppy = (size.y * STAGE_FILL / STAGE_HERO_HEIGHT).max(1.0);
    let dist = window.y / (2.0 * half_tan * ppy);
    let look = focus + Vec3::Y * (STAGE_HERO_HEIGHT * 0.5);
    let (so, co) = orbit.sin_cos();
    let (sp, cp) = STAGE_PITCH.sin_cos();
    let back = Vec3::new(so * cp, sp, co * cp);
    let eye = look + back * dist;
    let forward = -back;
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward);
    // Sliding the camera right moves the hero left on screen; screen y runs down.
    let slide =
        right * (-(center.x - window.x * 0.5) / ppy) + up * ((center.y - window.y * 0.5) / ppy);
    (eye + slide, look + slide)
}

/// A slow orbit at the menu; in play, the view `C` picked, on the hero where it now stands.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut rig: ResMut<CamRig>,
    player: Res<crate::player::Player>,
    world: WorldPoint,
    stage: Res<MenuStage>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    hero: Query<&Transform, (With<LocalHero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    // With no hero yet, the focus `drive_hero` grounded.
    let focus = hero.single().map_or(player.pos, |t| t.translation);
    let window = windows
        .single()
        .ok()
        .map(|w| Vec2::new(w.physical_width() as f32, w.physical_height() as f32))
        .filter(|w| w.x > 0.0 && w.y > 0.0);
    let staged = match (*phase.get(), stage.frame, window) {
        (Phase::Menu, Some(frame), Some(window)) => Some((frame, window)),
        _ => None,
    };
    let (eye, look) = if let Some((frame, window)) = staged {
        if stage.turning {
            rig.orbit += time.delta_secs() * 0.2;
        }
        let (mut eye, look) = stage_eye(focus, rig.orbit, frame, window);
        if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.5);
        }
        (eye, look)
    } else if matches!(phase.get(), Phase::Menu | Phase::Lobby) {
        rig.orbit += time.delta_secs() * 0.2;
        let d = 7.0;
        (
            focus + Vec3::new(rig.orbit.sin() * d, 2.4, rig.orbit.cos() * d),
            focus + Vec3::Y * 1.1,
        )
    } else if rig.mode == CamMode::Chase {
        chase_eye(&rig, focus, |p| world.terrain_height_under(p))
    } else {
        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        let look = focus + Vec3::Y * 1.4;
        let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
        // Never under the ground: a low tilt on a slope would bury the lens.
        if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.6);
        }
        (eye, look)
    };
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// The chase camera's eye and the point it looks at: a boom behind and above the hero's shoulders,
/// pulled in where a hill rises between them, and never under the ground.
fn chase_eye(rig: &CamRig, focus: Vec3, ground: impl Fn(Vec3) -> Option<f32>) -> (Vec3, Vec3) {
    let look = focus + Vec3::Y * CHASE_HEIGHT;
    let (sy, cy) = rig.yaw.sin_cos();
    let (sp, cp) = rig.pitch.sin_cos();
    let back = Vec3::new(cp * sy, sp, cp * cy);
    // Walk the boom outward; stop short of the first point under the terrain.
    const STEPS: usize = 12;
    let mut dist = rig.dist;
    for i in 1..=STEPS {
        let d = rig.dist * i as f32 / STEPS as f32;
        let p = look + back * d;
        if ground(p).is_some_and(|h| p.y < h + 0.4) {
            dist = (rig.dist * (i - 1) as f32 / STEPS as f32).max(1.2);
            break;
        }
    }
    let mut eye = look + back * dist;
    if let Some(h) = ground(eye) {
        eye.y = eye.y.max(h + 0.5);
    }
    // Looking up from low, aim a little above the shoulders so the hero stays in frame.
    let look = look + Vec3::Y * (-rig.pitch).max(0.0) * 2.0;
    (eye, look)
}

/// Damage the hero takes from one blow, after immunity, armour and the shield; returns what got
/// through.
pub(super) fn hurt_hero(hero: &mut Vitals, raw: f32) -> f32 {
    if hero.immune_t > 0.0 || hero.dead_t.is_some() {
        return 0.0;
    }
    let stats = hero.stats();
    // Armour takes a fixed amount off, then damage reduction a share of the rest.
    let mut dmg = (raw - stats.armor).max(1.0) * (1.0 - stats.damage_reduction);
    if hero.shield_t > 0.0 && hero.shield > 0.0 {
        let absorbed = dmg.min(hero.shield);
        hero.shield -= absorbed;
        dmg -= absorbed;
    }
    hero.hp -= dmg;
    dmg
}

/// How close an ally must stand over a fallen hero to bring them back, and for how long.
pub(super) const REVIVE_REACH: f32 = 3.0;
pub(super) const REVIVE_SECS: f32 = 4.0;
/// Seconds a fallen hero lies before the run counts them out, when no one can revive them.
const DOWN_SECS: f32 = 3.0;

/// The clock, every hero's regeneration, timed buffs and their visuals, death, revives, and the
/// run's outcome.
#[allow(clippy::type_complexity)]
pub(super) fn hero_vitals(
    time: Res<Time>,
    session: Res<Session>,
    mut run: ResMut<Run>,
    mut heroes: Query<
        (
            Entity,
            &Guid,
            &Transform,
            &mut ObjectStore,
            &mut Marks,
            &mut HeroState,
        ),
        With<Hero>,
    >,
    mut fields: MessageWriter<FieldChanged>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    let rules = session.rules;
    run.elapsed += dt;
    let standing: Vec<Vec3> = heroes
        .iter()
        .filter(|h| h.5.alive())
        .map(|h| h.2.translation)
        .collect();

    for (entity, guid, tf, mut store, mut marks, mut hero) in &mut heroes {
        let h = &mut hero.v;
        if let Some(t) = h.dead_t.as_mut() {
            *t += dt;
            let helped = rules.revive
                && standing
                    .iter()
                    .any(|p| reach(*p, tf.translation) <= REVIVE_REACH);
            h.revive_t = if helped {
                h.revive_t + dt
            } else {
                (h.revive_t - dt).max(0.0)
            };
            if h.revive_t >= REVIVE_SECS {
                h.dead_t = None;
                h.revive_t = 0.0;
                h.hp = h.stats().max_hp * 0.5;
                h.immune_t = 2.0;
                units::revive_unit(&mut store);
                fx.cast(entity, 2006, &[entity], None);
            }
            continue;
        }
        for t in [&mut h.shield_t, &mut h.immune_t, &mut h.thorns_t] {
            *t = (*t - dt).max(0.0);
        }
        // Effects that fire on their own: their visual plays on the hero as they do.
        for def in super::stats::tick_effects(&mut h.procs, &mut h.timed, dt) {
            if def.spell != 0 {
                fx.cast(entity, def.spell, &[entity], None);
            }
        }
        let stats = h.stats();
        let mut heal = stats.regen * dt;
        if h.heal_t > 0.0 {
            heal += h.heal_rate * dt.min(h.heal_t);
            h.heal_t = (h.heal_t - dt).max(0.0);
        }
        h.hp = (h.hp + heal).min(stats.max_hp);

        // The buffs show as their own state kits: the shield bubble, the immunity glow, the orbs.
        let mut shown: Vec<u32> = Vec::new();
        if h.shield_t > 0.0 && h.shield > 0.0 {
            shown.push(17);
        }
        if h.immune_t > 0.0 {
            shown.push(642);
        }
        if h.thorns_t > 0.0 {
            shown.push(h.thorns_spell);
        }
        if h.heal_t > 0.0 {
            shown.push(774);
        }
        for owned in &h.abilities {
            if owned.def.key == "lightning_shield" {
                shown.push(owned.def.spell);
            }
        }
        if h.passives.iter().any(|(p, _)| p.key == "renew") {
            shown.push(139);
        }
        for t in &h.timed {
            if t.spell != 0 && !shown.contains(&t.spell) {
                shown.push(t.spell);
            }
        }
        marks.set(&shown);
        marks.flush(entity, guid.0, &mut store, &mut fields);

        if h.hp <= 0.0 {
            h.hp = 0.0;
            h.dead_t = Some(0.0);
            h.revive_t = 0.0;
            units::kill_unit(&mut store);
            marks.clear();
            marks.flush(entity, guid.0, &mut store, &mut fields);
        }
    }

    if run.won || run.lost || heroes.is_empty() {
        return;
    }
    if run.final_slain {
        run.won = true;
        // A last flourish before the results.
        for (entity, .., hero) in &heroes {
            if hero.alive() {
                fx.cast(entity, 642, &[entity], None);
            }
        }
    } else if heroes
        .iter()
        .all(|h| h.5.dead_t.is_some_and(|t| t > DOWN_SECS))
    {
        run.lost = true;
    }
}

/// The run's last word sends everyone to the results.
pub(super) fn run_outcome(
    run: Res<Run>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
) {
    if (run.won || run.lost)
        && run.begun
        && matches!(phase.get(), Phase::Playing | Phase::LevelUp | Phase::Paused)
    {
        next.set(Phase::GameOver);
    }
}

/// Levels banked on the local hero open the cards.
pub(super) fn level_up_gate(
    run: Res<Run>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() == Phase::Playing
        && run.begun
        && run.pending_levels > 0
        && run.dead_t.is_none()
        && !run.won
        && !run.lost
    {
        next.set(Phase::LevelUp);
    }
}

/// The share of the run speed for running `dir` while facing `look`: the backward speed for any
/// run with a rearward part, as WoW's backward key takes it over strafe; full speed otherwise.
fn pace(look: Option<Vec3>, dir: Vec3) -> f32 {
    match look {
        Some(look) if look.dot(dir) < -1e-3 => RUN_BACK_RATIO,
        _ => 1.0,
    }
}

/// The movement flags for running `dir` while facing `look`: forward, backpedal or strafe, by
/// which quarter the run falls in, so the walk, backpedal and strafe animations play.
fn gait(look: Vec3, dir: Vec3) -> u32 {
    let ahead = look.dot(dir);
    // Positive when `dir` is to the hero's left: WoW faces −Z with +X on the right.
    let left = look.cross(dir).y;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

/// The ground-plane distance from the hero, the measure every range in the mode uses.
pub(super) fn reach(hero: Vec3, other: Vec3) -> f32 {
    flat(other - hero).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chase_view_sits_behind_and_looks_ahead() {
        let rig = CamRig::default();
        assert_eq!(rig.mode, CamMode::Chase);
        let (eye, look) = chase_eye(&rig, Vec3::ZERO, |_| Some(0.0));
        // Behind the hero (yaw 0 looks along -Z, so the camera is on +Z) and above.
        assert!(eye.z > 0.0 && eye.y > 0.0);
        let fwd = flat(look - eye).normalize();
        assert!(fwd.distance(rig.forward()) < 1e-4);
    }

    #[test]
    fn backpedalling_and_its_diagonals_go_at_the_backward_speed() {
        let look = Vec3::NEG_Z;
        let fwd = Vec3::NEG_Z;
        let back = Vec3::Z;
        let right = Vec3::X;
        assert_eq!(pace(Some(look), fwd), 1.0);
        assert_eq!(pace(Some(look), right), 1.0, "a strafe is full speed");
        assert_eq!(pace(Some(look), (fwd + right).normalize()), 1.0);
        assert_eq!(pace(Some(look), back), RUN_BACK_RATIO);
        assert_eq!(pace(Some(look), (back + right).normalize()), RUN_BACK_RATIO);
        // Facing where it runs, the hero never backpedals.
        assert_eq!(pace(None, back), 1.0);
    }

    #[test]
    fn the_stage_frames_the_hero_where_the_screen_asks() {
        let window = Vec2::new(1920.0, 1080.0);
        let focus = Vec3::new(10.0, 5.0, -3.0);
        let mid = focus + Vec3::Y * (STAGE_HERO_HEIGHT * 0.5);
        // Projects `p` through a camera at `eye` looking at `look`, to pixels.
        let project = |eye: Vec3, look: Vec3, p: Vec3| {
            let fwd = (look - eye).normalize();
            let right = fwd.cross(Vec3::Y).normalize();
            let up = right.cross(fwd);
            let v = p - eye;
            let depth = v.dot(fwd);
            let half_tan = (benilla_world::view::CAM_FOVY * 0.5).tan();
            let ndc = Vec2::new(
                v.dot(right) / (depth * half_tan * window.x / window.y),
                v.dot(up) / (depth * half_tan),
            );
            Vec2::new(
                (ndc.x + 1.0) * 0.5 * window.x,
                (1.0 - ndc.y) * 0.5 * window.y,
            )
        };
        for frame in [
            (Vec2::new(960.0, 540.0), Vec2::new(800.0, 900.0)),
            (Vec2::new(500.0, 300.0), Vec2::new(150.0, 210.0)),
        ] {
            let (eye, look) = stage_eye(focus, 0.7, frame, window);
            let at = project(eye, look, mid);
            assert!(at.distance(frame.0) < 1.0, "{at} for {frame:?}");
            // The hero's height fills the frame's share.
            let head = project(eye, look, mid + Vec3::Y * (STAGE_HERO_HEIGHT * 0.5));
            let tall = (at.y - head.y) * 2.0;
            assert!(
                (tall - frame.1.y * STAGE_FILL).abs() < frame.1.y * 0.05,
                "{tall}"
            );
        }
    }

    #[test]
    fn wasd_follows_the_camera_heading() {
        let mut rig = CamRig::default();
        rig.yaw = std::f32::consts::FRAC_PI_2;
        // W runs where the camera looks; D runs to its right.
        let w = rig.relative(Vec3::new(0.0, 0.0, -1.0));
        assert!(w.distance(rig.forward()) < 1e-5);
        let d = rig.relative(Vec3::new(1.0, 0.0, 0.0));
        assert!(d.dot(rig.forward()).abs() < 1e-5);
        assert!(rig.forward().cross(d).y < 0.0, "D is to the right");
    }

    #[test]
    fn a_hill_behind_pulls_the_boom_in() {
        let rig = CamRig::default();
        // A wall of ground rising behind the hero, 3 yd back.
        let hill = |p: Vec3| Some(if p.z > 3.0 { 50.0 } else { 0.0 });
        let (eye, look) = chase_eye(&rig, Vec3::ZERO, hill);
        assert!(eye.z <= 3.5, "the camera stays in front of the hill: {eye}");
        assert!(eye.distance(look) < rig.dist);
    }

    #[test]
    fn armour_then_damage_reduction_soften_a_hit() {
        use crate::survivors::data::{Bonus, Stat};
        let class = &data::CLASSES[0];
        let mut v = Vitals::new(class, 1).with_meta(
            vec![
                Bonus::flat(Stat::Armor, 4.0),
                Bonus::flat(Stat::DamageReduction, 0.25),
            ],
            &[],
        );
        let hp = v.hp;
        let taken = hurt_hero(&mut v, 20.0);
        assert!((taken - (20.0 - 4.0) * 0.75).abs() < 1e-5, "{taken}");
        assert!((v.hp - (hp - taken)).abs() < 1e-5);
        // Reduction never goes past its cap.
        let mut tank =
            Vitals::new(class, 1).with_meta(vec![Bonus::flat(Stat::DamageReduction, 5.0)], &[]);
        let taken = hurt_hero(&mut tank, 100.0);
        assert!((taken - 100.0 * (1.0 - data::MAX_DAMAGE_REDUCTION)).abs() < 1e-4);
    }

    #[test]
    fn what_a_hero_brings_in_counts_from_the_first_frame() {
        use crate::survivors::data::{Bonus, Stat};
        let class = &data::CLASSES[0];
        let bare = Vitals::new(class, 1);
        let geared = Vitals::new(class, 1).with_meta(vec![Bonus::flat(Stat::MaxHp, 25.0)], &[]);
        assert_eq!(geared.stats().max_hp, bare.stats().max_hp + 25.0);
        // ...and it starts at that full health.
        assert_eq!(geared.hp, geared.stats().max_hp);
    }

    #[test]
    fn views_cycle_and_reset_their_framing() {
        let mut rig = CamRig::default();
        rig.yaw = 1.0;
        rig.switch(rig.mode.next());
        assert_eq!(rig.mode, CamMode::Fixed);
        assert_eq!(rig.yaw, 0.0);
        rig.switch(rig.mode.next());
        assert_eq!(rig.mode, CamMode::Orbit);
        rig.switch(rig.mode.next());
        assert_eq!(rig.mode, CamMode::Chase);
        assert_eq!(rig.pitch, CHASE_PITCH);
    }
}
