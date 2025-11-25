//! Biome classification utilities shared by the surface streamer and renderer.
//!
//! The Planetary Annihilation style workflow expects a stable biome id per
//! location that every LOD and virtual texture lookup can agree on.  We model
//! that here as a lightweight classifier built on top of the existing climate
//! sampling code.  The classifier operates entirely in terms of the packed
//! attributes returned by [`ClimateModel::sample`], so it remains deterministic
//! for both procedural and baked patches.

use bevy::prelude::*;

use crate::game::world::planet::{ClimateModel, PlanetClimateSample, SKIRT_LAND_MASK_BITS};

/// Fixed palette of biome identifiers.  The values are stable so they can be
/// written to disk or sent over the network if needed.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeId {
    Ocean = 0,
    Coast = 1,
    Wetland = 2,
    Grassland = 3,
    Forest = 4,
    Desert = 5,
    Savannah = 6,
    Mountain = 7,
    Snow = 8,
    Glacier = 9,
}

impl BiomeId {
    pub const COUNT: usize = 10;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => BiomeId::Ocean,
            1 => BiomeId::Coast,
            2 => BiomeId::Wetland,
            3 => BiomeId::Grassland,
            4 => BiomeId::Forest,
            5 => BiomeId::Desert,
            6 => BiomeId::Savannah,
            7 => BiomeId::Mountain,
            8 => BiomeId::Snow,
            _ => BiomeId::Glacier,
        }
    }

    #[inline]
    pub fn palette_token(self) -> &'static str {
        match self {
            BiomeId::Ocean => "palette:default",
            BiomeId::Coast => "palette:default",
            BiomeId::Wetland => "palette:lush",
            BiomeId::Grassland => "palette:lush",
            BiomeId::Forest => "palette:lush",
            BiomeId::Desert => "palette:desert",
            BiomeId::Savannah => "palette:arid",
            BiomeId::Mountain => "palette:default",
            BiomeId::Snow => "palette:ice",
            BiomeId::Glacier => "palette:ice",
        }
    }

    #[inline]
    pub fn display_name(self) -> &'static str {
        match self {
            BiomeId::Ocean => "Ocean",
            BiomeId::Coast => "Coast",
            BiomeId::Wetland => "Wetland",
            BiomeId::Grassland => "Grassland",
            BiomeId::Forest => "Forest",
            BiomeId::Desert => "Desert",
            BiomeId::Savannah => "Savannah",
            BiomeId::Mountain => "Mountain",
            BiomeId::Snow => "Snow",
            BiomeId::Glacier => "Glacier",
        }
    }
}

/// Aggregated biome coverage information for a patch.
#[derive(Clone, Debug)]
pub struct PatchBiomeStats {
    pub counts: [u32; BiomeId::COUNT],
    pub coverage: [f32; BiomeId::COUNT],
    pub dominant: BiomeId,
}

impl PatchBiomeStats {
    pub fn from_vertices(vertices: &[BiomeId]) -> Self {
        let mut counts = [0u32; BiomeId::COUNT];
        for &biome in vertices {
            counts[biome.as_index()] = counts[biome.as_index()].saturating_add(1);
        }
        let total = vertices.len().max(1) as f32;
        let mut coverage = [0.0; BiomeId::COUNT];
        for (dst, &count) in coverage.iter_mut().zip(counts.iter()) {
            *dst = count as f32 / total;
        }
        let (dominant_index, _) = counts
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .unwrap_or((0, &0));

        Self {
            counts,
            coverage,
            dominant: BiomeId::from_index(dominant_index),
        }
    }
}

/// Converts climate samples into biome identifiers.  Thresholds are tuned for
/// the current procedural generation parameters and can be revisited when we
/// expand the biome set.
#[derive(Clone)]
pub struct BiomeClassifier {
    pub ocean_land_cutoff: f32,
    pub coast_threshold: f32,
    pub snow_threshold: f32,
    pub desert_moisture_threshold: f32,
    pub savannah_moisture_threshold: f32,
    pub forest_moisture_threshold: f32,
    pub mountain_slope_threshold: f32,
    pub mountain_height_threshold: f32,
}

impl Default for BiomeClassifier {
    fn default() -> Self {
        Self {
            ocean_land_cutoff: 0.52,
            coast_threshold: 0.55,
            snow_threshold: 0.62,
            desert_moisture_threshold: 0.32,
            savannah_moisture_threshold: 0.42,
            forest_moisture_threshold: 0.6,
            mountain_slope_threshold: 0.58,
            mountain_height_threshold: 0.18,
        }
    }
}

impl BiomeClassifier {
    #[inline]
    pub fn classify_positions(&self, positions: &[Vec3], climate: &ClimateModel) -> Vec<BiomeId> {
        positions
            .iter()
            .map(|pos| {
                let dir = pos.normalize_or_zero();
                let sample = climate.sample(dir);
                self.classify_sample(&sample)
            })
            .collect()
    }

    #[inline]
    pub fn classify_sample(&self, sample: &PlanetClimateSample) -> BiomeId {
        let (moisture, dryness) = unpack_pair(sample.uv1[0]);
        let (coast_band, snow_score) = unpack_pair(sample.uv1[1]);
        let (height01, temperature) = unpack_pair(sample.packed[0]);
        let (slope, continent) = unpack_pair(sample.packed[1]);
        let (mountain_mask, _) = unpack_pair(sample.packed[2]);
        let (_, land_mask) = unpack_depth_and_land(sample.packed[3]);

        if land_mask < self.ocean_land_cutoff {
            return BiomeId::Ocean;
        }

        if snow_score > self.snow_threshold || temperature < 0.18 {
            return if mountain_mask > 0.5 || height01 > self.mountain_height_threshold {
                BiomeId::Glacier
            } else {
                BiomeId::Snow
            };
        }

        if coast_band > self.coast_threshold && land_mask < (self.ocean_land_cutoff + 0.2) {
            return BiomeId::Coast;
        }

        if mountain_mask > 0.55
            || slope > self.mountain_slope_threshold
            || height01 > self.mountain_height_threshold
        {
            return BiomeId::Mountain;
        }

        if moisture < self.desert_moisture_threshold && dryness > 0.35 {
            return BiomeId::Desert;
        }

        if moisture < self.savannah_moisture_threshold && dryness > 0.25 {
            return BiomeId::Savannah;
        }

        if moisture > self.forest_moisture_threshold {
            return if temperature < 0.32 || continent < 0.4 {
                BiomeId::Wetland
            } else {
                BiomeId::Forest
            };
        }

        BiomeId::Grassland
    }
}

#[inline]
fn unpack_pair(value: f32) -> (f32, f32) {
    let bits = value.to_bits();
    let high = ((bits >> 16) & 0xFFFF) as f32 / 65535.0;
    let low = (bits & 0xFFFF) as f32 / 65535.0;
    (high, low)
}

#[inline]
fn unpack_depth_and_land(value: f32) -> (f32, f32) {
    let bits = value.to_bits();
    let depth = ((bits >> 16) & 0xFFFF) as f32 / 65535.0;
    let land_mask = (bits & SKIRT_LAND_MASK_BITS) as f32 / SKIRT_LAND_MASK_BITS as f32;
    (depth, land_mask)
}
