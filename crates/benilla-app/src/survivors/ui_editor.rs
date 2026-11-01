//! Native development-only controls and file watcher for the Survivors action-bar layout.
//!
//! Open the existing dev overlay (the dev chord + D) to show the editor. The same layout can be
//! adjusted here or in `ui-layouts/action_bar.toml`; changes are applied to the live HUD.

use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use benilla_world::dev_state::DebugState;

use super::frames::ActionBarLayout;

#[derive(Resource)]
pub(super) struct LayoutFileWatcher {
    path: PathBuf,
    poll: Timer,
    last_contents: Option<String>,
    status: String,
}

impl Default for LayoutFileWatcher {
    fn default() -> Self {
        let path = std::env::var_os("WOW_SURVIVORS_UI_LAYOUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("ui-layouts/action_bar.toml"));
        Self {
            path,
            poll: Timer::from_seconds(0.25, TimerMode::Repeating),
            last_contents: None,
            status: "Waiting for the layout file".to_string(),
        }
    }
}

pub(super) fn load_action_bar_layout(
    mut layout: ResMut<ActionBarLayout>,
    mut watcher: ResMut<LayoutFileWatcher>,
) {
    reload_from_disk(&mut layout, &mut watcher);
}

pub(super) fn watch_action_bar_layout(
    time: Res<Time>,
    mut layout: ResMut<ActionBarLayout>,
    mut watcher: ResMut<LayoutFileWatcher>,
) {
    watcher.poll.tick(time.delta());
    if !watcher.poll.just_finished() {
        return;
    }

    let Ok(contents) = fs::read_to_string(&watcher.path) else {
        return;
    };
    if watcher.last_contents.as_deref() == Some(contents.as_str()) {
        return;
    }

    apply_file_contents(contents, &mut layout, &mut watcher);
}

fn reload_from_disk(layout: &mut ActionBarLayout, watcher: &mut LayoutFileWatcher) {
    match fs::read_to_string(&watcher.path) {
        Ok(contents) => apply_file_contents(contents, layout, watcher),
        Err(error) => {
            watcher.last_contents = None;
            watcher.status = format!("Could not read {}: {error}", watcher.path.display());
            warn!("{}", watcher.status);
        }
    }
}

fn apply_file_contents(
    contents: String,
    layout: &mut ActionBarLayout,
    watcher: &mut LayoutFileWatcher,
) {
    watcher.last_contents = Some(contents.clone());
    match toml::from_str::<ActionBarLayout>(&contents) {
        Ok(candidate) if candidate.is_valid() => {
            if *layout != candidate {
                *layout = candidate;
            }
            watcher.status = format!("Loaded {}", watcher.path.display());
            info!("survivors HUD layout reloaded from {}", watcher.path.display());
        }
        Ok(_) => {
            watcher.status = "Invalid layout values; keeping the last valid layout".to_string();
            warn!("survivors HUD layout contains values outside the editor's safe ranges");
        }
        Err(error) => {
            watcher.status = format!("Invalid TOML; keeping the last valid layout: {error}");
            warn!("survivors HUD layout TOML could not be parsed: {error}");
        }
    }
}

fn save_to_disk(layout: &ActionBarLayout, watcher: &mut LayoutFileWatcher) {
    let contents = match toml::to_string_pretty(layout) {
        Ok(contents) => contents,
        Err(error) => {
            watcher.status = format!("Could not serialize layout: {error}");
            return;
        }
    };

    if let Some(parent) = watcher.path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            watcher.status = format!("Could not create {}: {error}", parent.display());
            return;
        }
    }

    match fs::write(&watcher.path, &contents) {
        Ok(()) => {
            watcher.last_contents = Some(contents);
            watcher.status = format!("Saved {}", watcher.path.display());
        }
        Err(error) => {
            watcher.status = format!("Could not save {}: {error}", watcher.path.display());
        }
    }
}

/// The window shares the existing dev overlay camera and only appears while that overlay is open.
pub(super) fn draw_editor(
    mut contexts: EguiContexts,
    debug: Option<Res<DebugState>>,
    mut layout: ResMut<ActionBarLayout>,
    mut watcher: ResMut<LayoutFileWatcher>,
) -> Result {
    if !debug.is_some_and(|state| state.open) {
        return Ok(());
    }

    let ctx = contexts.ctx_mut()?;
    let mut draft = *layout;
    let mut save_requested = false;
    let mut reload_requested = false;
    let mut reset_requested = false;

    egui::Window::new("Survivors — Action Bar Layout")
        .default_width(340.0)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
        .show(ctx, |ui| {
            ui.label("Changes are applied to the live HUD immediately.");
            ui.label("Save writes the TOML file; external edits reload automatically.");
            ui.separator();

            layout_slider(ui, "Width", &mut draft.width, 270.0, 1200.0);
            layout_slider(ui, "Height", &mut draft.height, 48.0, 100.0);
            layout_slider(ui, "Horizontal offset", &mut draft.offset_x, -400.0, 400.0);
            layout_slider(ui, "Vertical offset", &mut draft.offset_y, -150.0, 150.0);

            ui.separator();
            ui.horizontal(|ui| {
                save_requested = ui.button("Save layout").clicked();
                reload_requested = ui.button("Reload file").clicked();
                reset_requested = ui.button("Reset").clicked();
            });
            ui.weak(format!("File: {}", watcher.path.display()));
            ui.weak(&watcher.status);
        });

    if reset_requested {
        draft = ActionBarLayout::default();
    }
    if draft != *layout {
        *layout = draft;
    }
    if save_requested {
        save_to_disk(&layout, &mut watcher);
    }
    if reload_requested {
        reload_from_disk(&mut layout, &mut watcher);
    }
    Ok(())
}

fn layout_slider(ui: &mut egui::Ui, label: &str, value: &mut f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(value, min..=max).text(label));
        ui.add(egui::DragValue::new(value).range(min..=max).speed(1.0));
    });
}
