use bevy::prelude::*;

// bring modules into the crate root so `crate::menu` / `crate::game` resolve
mod app;
mod menu;
mod game;
mod core;

use app::AppState;



fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<AppState>()
        .add_plugins(menu::MenuPlugin)
        .add_plugins(core::camera::EditorCameraPlugin) // ← add this
        .add_plugins(game::GamePlugin)
        .run();
}