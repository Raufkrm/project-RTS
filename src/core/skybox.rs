use bevy::asset::RenderAssetUsages;
use bevy::math::primitives::Sphere;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, Face, TextureDimension, TextureFormat, TextureUsages,
};

const STAR_TEXTURE_WIDTH: u32 = 2048;
const STAR_TEXTURE_HEIGHT: u32 = 1024;
const STAR_DENSITY: f32 = 0.0008;

pub struct SkyboxPlugin;

#[derive(Resource)]
pub struct StarfieldAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct Skybox;

impl Plugin for SkyboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, prepare_starfield_assets);
    }
}

fn prepare_starfield_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let texture = generate_starfield_image(STAR_TEXTURE_WIDTH, STAR_TEXTURE_HEIGHT, 0xC0FFEE);
    let texture_handle = images.add(texture);

    let mesh_handle = meshes.add(Sphere::new(1.0));

    let material_handle = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        base_color_texture: Some(texture_handle),
        unlit: true,
        cull_mode: Some(Face::Front),
        double_sided: true,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });

    commands.insert_resource(StarfieldAssets {
        mesh: mesh_handle,
        material: material_handle,
    });
}

fn generate_starfield_image(width: u32, height: u32, seed: u64) -> Image {
    let mut data = vec![0u8; (width * height * 4) as usize];
    let mut rng = Lcg::new(seed);

    // base background
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let base_noise = (rng.next_f32() * 6.0) as u8;
            data[idx] = base_noise;
            data[idx + 1] = (base_noise as f32 * 0.9) as u8;
            data[idx + 2] = (base_noise as f32 * 1.1).min(255.0) as u8;
            data[idx + 3] = 255;

            if rng.next_f32() < STAR_DENSITY {
                let hue_pick = rng.next_f32();
                let intensity = 180.0 + rng.next_f32() * 75.0;
                let (r, g, b) = if hue_pick < 0.33 {
                    (intensity, intensity * 0.92, intensity * 0.85)
                } else if hue_pick < 0.66 {
                    (intensity * 0.8, intensity * 0.9, intensity)
                } else {
                    (intensity, intensity, intensity)
                };
                paint_star(
                    &mut data,
                    width,
                    height,
                    x as i32,
                    y as i32,
                    [r as u8, g as u8, b as u8],
                );
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
    image
}

fn paint_star(data: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 3]) {
    const STAR_PATTERN: &[(i32, i32, f32)] = &[
        (0, 0, 1.0),
        (1, 0, 0.3),
        (-1, 0, 0.3),
        (0, 1, 0.3),
        (0, -1, 0.3),
    ];

    for (dx, dy, weight) in STAR_PATTERN.iter().copied() {
        blend_pixel(data, width, height, x + dx, y + dy, color, weight);
    }
}

fn blend_pixel(
    data: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    color: [u8; 3],
    weight: f32,
) {
    if height == 0 || width == 0 {
        return;
    }

    let wrapped_x = ((x % width as i32) + width as i32) % width as i32;
    let clamped_y = y.clamp(0, height as i32 - 1);
    let idx = ((clamped_y as u32 * width + wrapped_x as u32) * 4) as usize;
    for c in 0..3 {
        let existing = data[idx + c] as f32;
        let added = color[c] as f32 * weight;
        data[idx + c] = (existing + added).min(255.0) as u8;
    }
    data[idx + 3] = 255;
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
}
