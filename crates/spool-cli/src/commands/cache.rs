//! Session cache - stores converted SpoolFiles to avoid re-parsing unchanged logs.
//!
//! Cache entries are stored in `~/.cache/spool/` (or platform equivalent) as `.spool` files,
//! named by a hash of the source path. Each entry includes a metadata sidecar with the
//! source file's mtime, so we can detect when the cache is stale.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use spool_format::SpoolFile;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata stored alongside each cached spool file.
#[derive(Serialize, Deserialize)]
struct CacheMeta {
    /// Modification time of the source file when cached.
    source_mtime_secs: u64,
    /// Size of the source file when cached.
    source_size: u64,
}

/// Get the cache directory, creating it if necessary.
fn cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from(".cache"));
    let cache_path = base.join("spool").join("sessions");
    if !cache_path.exists() {
        fs::create_dir_all(&cache_path)
            .with_context(|| format!("Failed to create cache directory: {:?}", cache_path))?;
    }
    Ok(cache_path)
}

/// Generate a cache key from a source path.
fn cache_key(source: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    source.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Get source file metadata (mtime, size) for cache validation.
fn source_metadata(source: &Path) -> Result<(u64, u64)> {
    let meta = fs::metadata(source).with_context(|| format!("Failed to stat {:?}", source))?;
    let mtime = meta
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok((mtime, meta.len()))
}

/// Try to load a cached SpoolFile for the given source path.
/// Returns None if not cached or if the cache is stale.
pub fn load_cached(source: &Path) -> Option<SpoolFile> {
    let cache_path = cache_dir().ok()?;
    let key = cache_key(source);
    let spool_path = cache_path.join(format!("{}.spool", key));
    let meta_path = cache_path.join(format!("{}.meta", key));

    // Check if cache files exist
    if !spool_path.exists() || !meta_path.exists() {
        return None;
    }

    // Load and validate metadata
    let meta_json = fs::read_to_string(&meta_path).ok()?;
    let meta: CacheMeta = serde_json::from_str(&meta_json).ok()?;

    // Check if source has changed
    let (current_mtime, current_size) = source_metadata(source).ok()?;
    if meta.source_mtime_secs != current_mtime || meta.source_size != current_size {
        // Cache is stale, remove it
        let _ = fs::remove_file(&spool_path);
        let _ = fs::remove_file(&meta_path);
        return None;
    }

    // Load the cached spool file
    SpoolFile::from_path(&spool_path).ok()
}

/// Save a SpoolFile to the cache for the given source path.
pub fn save_cached(source: &Path, spool: &SpoolFile) -> Result<()> {
    let cache_path = cache_dir()?;
    let key = cache_key(source);
    let spool_path = cache_path.join(format!("{}.spool", key));
    let meta_path = cache_path.join(format!("{}.meta", key));

    // Get source metadata
    let (mtime, size) = source_metadata(source)?;
    let meta = CacheMeta {
        source_mtime_secs: mtime,
        source_size: size,
    };

    // Write the spool file
    spool
        .write_to_path(&spool_path)
        .with_context(|| format!("Failed to write cache: {:?}", spool_path))?;

    // Write the metadata
    let meta_json = serde_json::to_string(&meta)?;
    fs::write(&meta_path, meta_json)
        .with_context(|| format!("Failed to write cache meta: {:?}", meta_path))?;

    Ok(())
}

/// Load a session with caching. If the source is already a .spool file, loads directly.
/// Otherwise, checks the cache first, then converts and caches if needed.
pub fn load_cached_or_convert<F>(source: &Path, convert_fn: F) -> Result<SpoolFile>
where
    F: FnOnce() -> Result<SpoolFile>,
{
    // .spool files don't need caching
    if source.extension().map(|e| e == "spool").unwrap_or(false) {
        return SpoolFile::from_path(source)
            .with_context(|| format!("Failed to read: {:?}", source));
    }

    // Try cache first
    if let Some(cached) = load_cached(source) {
        return Ok(cached);
    }

    // Convert and cache
    let spool = convert_fn()?;

    // Best-effort cache save (don't fail if caching fails)
    if let Err(e) = save_cached(source, &spool) {
        eprintln!("Warning: failed to cache session: {}", e);
    }

    Ok(spool)
}
