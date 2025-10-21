mod assets;

use assets::{resolve_model_visuals, setup_fallback_assets};
use bevy::prelude::*;

#[allow(unused_imports)]
pub use assets::{
    FallbackAssets, MissingModelPlaceholder, ModelScene, ModelSceneBundle, ModelVisualState,
};

pub struct LoadingPlugin;
impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_fallback_assets)
            .add_systems(Update, resolve_model_visuals);
    }
}
