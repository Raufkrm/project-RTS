use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

// --- make this public & at module scope ---
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}

pub fn build_app() -> App {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins.set(AssetPlugin {
            // keep the template's web-friendly setting
            meta_check: AssetMetaCheck::Never,
            ..default()
        })
    );

    app.init_state::<AppState>();

    app.add_plugins((
        crate::loading::LoadingPlugin,
        crate::menu::MenuPlugin,
        crate::game::GamePlugin,
    ));

    app
}
