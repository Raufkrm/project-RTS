<<<<<<< HEAD
pub mod biome;
pub mod virtual_texture;
=======
//! Planet surface LOD + streaming scaffolding.
//!
//! This module is intentionally skeletal: it captures the types and systems we
//! know we will need so the rest of the codebase can start referencing them
//! without pulling in unfinished logic.  The real implementation will land in
//! follow-up PRs.

pub mod asset_loader;
pub mod lod;
pub mod manager;
pub mod procedural_loader;
pub mod render;
pub mod stream;
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
