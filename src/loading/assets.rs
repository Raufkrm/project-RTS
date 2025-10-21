use bevy::{
    asset::{AssetPath, LoadState, RenderAssetUsages},
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// Mesh and material handles that can be re-used whenever a model fails to load.
#[derive(Resource, Clone)]
pub struct FallbackAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FallbackAssets {
    fn new(
        mut meshes: ResMut<Assets<Mesh>>,
        mut images: ResMut<Assets<Image>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) -> Self {
        let checkerboard_image = create_checkerboard_texture();
        let texture_handle = images.add(checkerboard_image);

        let material_handle = materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            unlit: true,
            ..default()
        });

        let mesh_handle = meshes.add(Mesh::from(Cuboid::from_size(Vec3::splat(1.0))));

        Self {
            mesh: mesh_handle,
            material: material_handle,
        }
    }
}

/// Component used to declare that an entity should be backed by the referenced [`Scene`] once loaded.
#[derive(Component, Clone)]
pub struct ModelScene {
    handle: Handle<Scene>,
}

impl ModelScene {
    pub fn new(handle: Handle<Scene>) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> &Handle<Scene> {
        &self.handle
    }

    pub fn clone_handle(&self) -> Handle<Scene> {
        self.handle.clone()
    }

    pub fn from_path(path: impl Into<AssetPath<'static>>, asset_server: &AssetServer) -> Self {
        Self::new(asset_server.load(path))
    }
}

impl From<Handle<Scene>> for ModelScene {
    fn from(value: Handle<Scene>) -> Self {
        Self::new(value)
    }
}

impl AsRef<Handle<Scene>> for ModelScene {
    fn as_ref(&self) -> &Handle<Scene> {
        self.handle()
    }
}

/// Marker added when the fallback visuals are active on an entity.
#[derive(Component, Default)]
pub struct MissingModelPlaceholder;

/// Internal component that tracks what visuals are currently attached to the entity.
#[derive(Component, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ModelVisualState {
    Pending,
    Scene,
    Fallback,
}

impl Default for ModelVisualState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Bundle that sets up an entity ready to be resolved by [`resolve_model_visuals`].
#[derive(Bundle)]
pub struct ModelSceneBundle {
    pub model: ModelScene,
    pub state: ModelVisualState,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
}

impl ModelSceneBundle {
    pub fn new(model: ModelScene, transform: Transform) -> Self {
        Self {
            model,
            state: ModelVisualState::Pending,
            transform,
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Visible,
            inherited_visibility: InheritedVisibility::VISIBLE,
            view_visibility: ViewVisibility::default(),
        }
    }
}

/// Creates the fallback mesh, material and texture up front.
pub fn setup_fallback_assets(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    images: ResMut<Assets<Image>>,
    materials: ResMut<Assets<StandardMaterial>>,
    existing: Option<Res<FallbackAssets>>,
) {
    if existing.is_none() {
        commands.insert_resource(FallbackAssets::new(meshes, images, materials));
    }
}

/// Resolves [`ModelScene`] components into either a loaded scene or a placeholder when loading fails.
///
/// Insert [`ModelScene`] (optionally via [`ModelSceneBundle`]) to any entity that should display a 3D scene.
/// The system will:
/// - attach a [`SceneRoot`] once the scene has finished loading,
/// - attach a purple/black checkerboard cube if the load fails,
/// - swap between the two if the load state later changes.
#[allow(clippy::too_many_arguments)]
pub fn resolve_model_visuals(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    fallback: Res<FallbackAssets>,
    mut query: Query<(
        Entity,
        &ModelScene,
        &mut ModelVisualState,
        Option<&MissingModelPlaceholder>,
    )>,
) {
    for (entity, model, mut state, has_placeholder) in &mut query {
        let Some(load_state) = asset_server.get_load_state(model.handle.id()) else {
            continue;
        };

        match load_state {
            LoadState::Loaded if *state != ModelVisualState::Scene => {
                commands
                    .entity(entity)
                    .insert(SceneRoot(model.clone_handle()));
                commands.entity(entity).remove::<(
                    Mesh3d,
                    MeshMaterial3d<StandardMaterial>,
                    MissingModelPlaceholder,
                )>();
                *state = ModelVisualState::Scene;
            }
            LoadState::Failed(_) if *state != ModelVisualState::Fallback => {
                commands.entity(entity).remove::<SceneRoot>();
                if has_placeholder.is_none() {
                    commands.entity(entity).insert((
                        Mesh3d(fallback.mesh.clone()),
                        MeshMaterial3d::<StandardMaterial>(fallback.material.clone()),
                        MissingModelPlaceholder,
                    ));
                }
                *state = ModelVisualState::Fallback;
            }
            _ => {}
        }
    }
}

fn create_checkerboard_texture() -> Image {
    const SIZE: u32 = 2;
    const TEXELS: [[u8; 4]; 2] = [[255, 0, 255, 255], [0, 0, 0, 255]];

    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let index = ((x + y) % TEXELS.len() as u32) as usize;
            data.extend_from_slice(&TEXELS[index]);
        }
    }

    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    image.sampler = ImageSampler::nearest();
    image
}
