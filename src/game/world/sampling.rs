use bevy::prelude::*;

#[derive(Clone, Copy)]
pub struct Sample {
    pub height: f32,
    pub temp: f32,
    pub humidity: f32,
}

pub trait WorldSampler: Send + Sync + 'static {
    fn sample(&self, x: f32, z: f32) -> Sample;

    #[inline]
    fn is_water(&self, s: &Sample, water_level: f32) -> bool {
        s.height < water_level
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlatSampler {
    pub seed: u64,
    pub base_freq: f32,
    pub height_amp: f32,
}

impl WorldSampler for FlatSampler {
    fn sample(&self, x: f32, z: f32) -> Sample {
        let n = fbm(self.seed, x * self.base_freq, z * self.base_freq);
        let h = (n - 0.5) * 2.0 * self.height_amp;

        let t = fbm(self.seed ^ 0xA1, x * 0.00030, z * 0.00030);
        let m = fbm(self.seed ^ 0xB2, x * 0.00050, z * 0.00050);

        Sample { height: h, temp: t, humidity: m }
    }
}

#[derive(Resource)]
pub struct FlatSamplerRes(pub FlatSampler);

// ---- tiny noise

#[inline]
fn h01(seed: u64, ix: i32, iz: i32) -> f32 {
    let mut v = seed
        ^ ((ix as u64).wrapping_mul(0x9E37_79B1_85EB_CA87))
        ^ ((iz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ceb9fe1a85ec53);
    v ^= v >> 33;
    (v as u32) as f32 / (u32::MAX as f32)
}
#[inline] fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }
#[inline] fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn value(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let z1 = z0 + 1;
    let tx = smooth(x - x.floor());
    let tz = smooth(z - z.floor());
    let v00 = h01(seed, x0, z0);
    let v10 = h01(seed, x1, z0);
    let v01 = h01(seed, x0, z1);
    let v11 = h01(seed, x1, z1);
    lerp(lerp(v00, v10, tx), lerp(v01, v11, tx), tz)
}

fn fbm(seed: u64, mut x: f32, mut z: f32) -> f32 {
    let mut amp = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..5 {
        sum += value(seed, x, z) * amp;
        norm += amp;
        amp *= 0.5;
        x *= 2.0;
        z *= 2.0;
    }
    (sum / norm).clamp(0.0, 1.0)
}
