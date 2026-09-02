use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::crypto;
use crate::model::Document;

pub fn data_path() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("TDEE_DATA_DIR") {
        return Ok(PathBuf::from(dir).join("tdee.json.age"));
    }
    let manifest_dir = std::env::current_dir().context("Cannot determine current directory")?;
    Ok(manifest_dir.join("data").join("tdee.json.age"))
}

pub fn load() -> Result<Document> {
    let path = data_path()?;
    let key = crypto::key_path()?;
    crypto::load_encrypted(&path, &key)
}

pub fn save(doc: &Document) -> Result<()> {
    let path = data_path()?;
    let key = crypto::key_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crypto::save_encrypted(doc, &path, &key)
}

pub fn data_file_exists() -> Result<bool> {
    Ok(data_path()?.exists())
}
