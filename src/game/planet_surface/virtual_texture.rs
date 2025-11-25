use bevy::asset::AssetServer;
use bevy::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlanetPatchId {
    pub face: u8,
    pub lod: u8,
    pub tile_x: u8,
    pub tile_y: u8,
}

impl PlanetPatchId {
    pub fn new(face: u8, lod: u8, tile_x: u8, tile_y: u8) -> Self {
        Self {
            face,
            lod,
            tile_x,
            tile_y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatchTextureSemantic {
    Albedo,
    Normal,
    Material,
    Roughness,
    Other,
}

impl PatchTextureSemantic {
    fn from_filename(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("albedo") || lower.contains("basecolor") || lower.contains("diffuse") {
            PatchTextureSemantic::Albedo
        } else if lower.contains("normal") {
            PatchTextureSemantic::Normal
        } else if lower.contains("material") || lower.contains("mask") {
            PatchTextureSemantic::Material
        } else if lower.contains("rough") || lower.contains("metal") {
            PatchTextureSemantic::Roughness
        } else {
            PatchTextureSemantic::Other
        }
    }
}

#[derive(Clone, Debug)]
pub struct PatchTextureFile {
    pub semantic: PatchTextureSemantic,
    pub asset_path: String,
}

#[derive(Clone, Debug)]
pub struct PlanetPatchPageMeta {
    pub id: PlanetPatchId,
    pub textures: Vec<PatchTextureFile>,
}

#[derive(Clone, Debug)]
pub struct PlanetPatchManifest {
    pub root: PathBuf,
    pub pages: Vec<PlanetPatchPageMeta>,
}

impl PlanetPatchManifest {
    pub fn scan(root: impl AsRef<Path>) -> Result<Self, PatchScanError> {
        let root = root.as_ref();
        if !root.exists() {
            return Err(PatchScanError::MissingRoot(root.to_path_buf()));
        }
        let mut pages = Vec::new();
        let assets_root = Path::new("assets");
        for face_entry in fs::read_dir(root)? {
            let face_entry = face_entry?;
            if !face_entry.file_type()?.is_dir() {
                continue;
            }
            let face_name = face_entry.file_name().to_string_lossy().into_owned();
            let Some(face_idx) = parse_face_dir(&face_name) else {
                warn!("Skipping unexpected planet patch folder '{face_name}'");
                continue;
            };
            for lod_entry in fs::read_dir(face_entry.path())? {
                let lod_entry = lod_entry?;
                if !lod_entry.file_type()?.is_dir() {
                    continue;
                }
                let lod_name = lod_entry.file_name().to_string_lossy().into_owned();
                let Some((lod, tile_x, tile_y)) = parse_lod_dir(&lod_name) else {
                    warn!(
                        "Skipping unexpected LOD folder '{lod_name}' inside {}",
                        face_entry.path().display()
                    );
                    continue;
                };
                let textures_dir = lod_entry.path().join("textures");
                if !textures_dir.exists() {
                    warn!(
                        "Planet patch {} missing textures directory at {}",
                        lod_name,
                        textures_dir.display()
                    );
                    continue;
                }
                let mut textures = Vec::new();
                for file in fs::read_dir(&textures_dir)? {
                    let file = file?;
                    if !file.file_type()?.is_file() {
                        continue;
                    }
                    let rel_path = file.path();
                    let asset_rel = rel_path
                        .strip_prefix(assets_root)
                        .or_else(|_| {
                            // when assets/ prefix is missing fall back to stripping root
                            rel_path.strip_prefix(root.parent().unwrap_or(root))
                        })
                        .unwrap_or(&rel_path);
                    let normalized = asset_rel.to_string_lossy().replace('\\', "/");
                    let semantic =
                        PatchTextureSemantic::from_filename(&file.file_name().to_string_lossy());
                    textures.push(PatchTextureFile {
                        semantic,
                        asset_path: normalized,
                    });
                }
                if textures.is_empty() {
                    continue;
                }
                pages.push(PlanetPatchPageMeta {
                    id: PlanetPatchId::new(face_idx, lod, tile_x, tile_y),
                    textures,
                });
            }
        }

        pages.sort_by_key(|meta| meta.id);
        Ok(Self {
            root: root.to_path_buf(),
            pages,
        })
    }

    pub fn page(&self, id: &PlanetPatchId) -> Option<&PlanetPatchPageMeta> {
        self.pages.iter().find(|meta| &meta.id == id)
    }

    pub fn pages_for_face(&self, face: u8) -> impl Iterator<Item = &PlanetPatchPageMeta> {
        self.pages.iter().filter(move |meta| meta.id.face == face)
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

#[derive(Debug)]
pub enum PatchScanError {
    MissingRoot(PathBuf),
    Io(std::io::Error),
}

impl From<std::io::Error> for PatchScanError {
    fn from(value: std::io::Error) -> Self {
        PatchScanError::Io(value)
    }
}

impl fmt::Display for PatchScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchScanError::MissingRoot(path) => {
                write!(f, "planet patch root '{}' not found", path.display())
            }
            PatchScanError::Io(err) => err.fmt(f),
        }
    }
}

impl Error for PatchScanError {}

#[derive(Clone)]
pub struct PlanetPatchPageHandles {
    pub id: PlanetPatchId,
    pub textures: Vec<(PatchTextureSemantic, Handle<Image>)>,
}

#[derive(Resource)]
pub struct PlanetPatchCache {
    manifest: PlanetPatchManifest,
    resident: HashMap<PlanetPatchId, PlanetPatchPageHandles>,
}

impl PlanetPatchCache {
    pub fn new(manifest: PlanetPatchManifest) -> Self {
        Self {
            manifest,
            resident: HashMap::new(),
        }
    }

    pub fn manifest(&self) -> &PlanetPatchManifest {
        &self.manifest
    }

    pub fn resident_count(&self) -> usize {
        self.resident.len()
    }

    pub fn ensure_page(
        &mut self,
        id: PlanetPatchId,
        asset_server: &AssetServer,
    ) -> Option<&PlanetPatchPageHandles> {
        if !self.resident.contains_key(&id) {
            let meta = self.manifest.page(&id)?;
            let textures = meta
                .textures
                .iter()
                .map(|tex| {
                    (
                        tex.semantic,
                        asset_server.load::<Image>(tex.asset_path.clone()),
                    )
                })
                .collect();
            self.resident
                .insert(id, PlanetPatchPageHandles { id, textures });
        }
        self.resident.get(&id)
    }

    pub fn preload_face(&mut self, face: u8, asset_server: &AssetServer) {
        let ids: Vec<_> = self
            .manifest
            .pages_for_face(face)
            .map(|meta| meta.id)
            .collect();
        for id in ids {
            let _ = self.ensure_page(id, asset_server);
        }
    }
}

fn parse_face_dir(name: &str) -> Option<u8> {
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("face_") {
        return None;
    }
    lower[5..].parse().ok()
}

fn parse_lod_dir(name: &str) -> Option<(u8, u8, u8)> {
    let mut parts = name.split('_');
    let lod_part = parts.next()?;
    if !lod_part.to_ascii_lowercase().starts_with("lod") {
        return None;
    }
    let lod = lod_part[3..].parse().ok()?;
    let tile_x = parts.next()?.parse().ok()?;
    let tile_y = parts.next()?.parse().ok()?;
    Some((lod, tile_x, tile_y))
}
