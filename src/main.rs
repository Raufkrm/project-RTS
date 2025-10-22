use bevy::prelude::*;

// bring modules into the crate root so crate::menu / crate::game resolve
mod app;
mod core;
mod game;
<<<<<<< HEAD
=======
mod loading;
>>>>>>> 7e6f9f8ca734934589b0e887862f7c5feb852eed
mod menu;

use app::AppState;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(loading::LoadingPlugin)
        .init_state::<AppState>()
        .add_plugins(menu::MenuPlugin)
<<<<<<< HEAD
=======
        .add_plugins(core::camera::EditorCameraPlugin) // add this
>>>>>>> 7e6f9f8ca734934589b0e887862f7c5feb852eed
        .add_plugins(game::GamePlugin)
        .run();
}
