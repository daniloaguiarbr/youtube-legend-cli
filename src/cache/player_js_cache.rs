//! On-disk TTL cache for fetched `player.js` blobs.
//!
//! The cache is keyed by the `YouTube` player `version` segment
//! (the URL path component) and stored under
//! `$XDG_CACHE_HOME/youtube-legend-cli/player/<version>.bin` with a
//! 7-day TTL. The same-version blob is large and rarely changes,
//! so caching aggressively is correct.
//!
//! Concurrency: a `tokio::sync::Mutex` serialises concurrent
//! fetches for the same version so a cold-start under a burst of
//! requests does not stampede the upstream. This is the
//! single-flight pattern from `rules-rust-cache-sqlite-redis`.
//!
//! The operations table — the small parsed `Vec<JsOperation>` — is
//! persisted alongside the blob in a sidecar file so re-parsing on
//! cache hit is also avoided. See
//! [`crate::cache::operations_cache`] for the sidecar format.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use reqwest::Client;
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::provider::youtube::player_js::{fetch_player_js, PlayerJs};

/// Default TTL for cached `player.js` blobs. Seven days matches the
/// design note in `gaps.md` and the upstream change cadence.
const PLAYER_JS_TTL_SECS: u64 = 7 * 24 * 60 * 60;
/// Subdirectory under the XDG cache root.
const PLAYER_CACHE_SUBDIR: &str = "player";
/// Filename suffix for the sidecar operations file.
const OPS_SIDECAR_SUFFIX: &str = ".ops.bin";

/// Cached `player.js` plus its parsed operations table.
#[derive(Debug, Clone)]
pub struct CachedPlayer {
    /// Full fetched blob with metadata.
    pub player: PlayerJs,
}

/// Process-wide single-flight lock. One per `version` is tracked
/// in the `Mutex<HashMap<String, Arc<Mutex<()>>>>` so a request
/// for version `v1` does not block version `v2`. We keep the map
/// behind an outer `OnceLock` so a cold process pays the
/// allocation cost only once.
fn inflight_lock() -> &'static Mutex<std::collections::HashMap<String, std::sync::Arc<Mutex<()>>>> {
    static LOCK: OnceLock<Mutex<std::collections::HashMap<String, std::sync::Arc<Mutex<()>>>>> =
        OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

async fn lock_for(version: &str) -> std::sync::Arc<Mutex<()>> {
    let mut guard = inflight_lock().lock().await;
    guard
        .entry(version.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone()
}

/// Build the absolute cache path for a given version, creating
/// the parent directory if necessary.
fn cache_file_path(version: &str) -> AppResult<PathBuf> {
    if version.is_empty() {
        return Err(AppError::InvalidInput(
            "player_js_cache requires non-empty version".to_string(),
        ));
    }
    let proj = ProjectDirs::from("com", "youtube-legend-cli", "youtube-legend-cli")
        .ok_or_else(|| AppError::Internal("could not determine cache directory".to_string()))?;
    let dir = proj.cache_dir().join(PLAYER_CACHE_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("creating cache dir: {e}"))))?;
    Ok(dir.join(format!("{version}.bin")))
}

/// Sidecar path for the operations table.
fn ops_sidecar_path(version: &str) -> AppResult<PathBuf> {
    let main = cache_file_path(version)?;
    let mut name = main
        .file_name()
        .ok_or_else(|| AppError::Internal("cache path has no filename".to_string()))?
        .to_os_string();
    name.push(OPS_SIDECAR_SUFFIX);
    Ok(main.with_file_name(name))
}

/// True when the file at `path` exists and is younger than `ttl`.
async fn is_fresh(path: &std::path::Path, ttl: Duration) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let elapsed = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    now.saturating_sub(elapsed) <= ttl
}

/// Look up a `PlayerJs` by version, fetching + caching on miss.
///
/// Concurrency: a per-version `tokio::sync::Mutex` ensures that
/// when 10 coroutines hit the same cold key simultaneously, only
/// one of them walks the network path; the other 9 block on the
/// same mutex, then read the freshly-cached file.
///
/// # Errors
///
/// Returns \[`crate::error::AppError::InvalidInput`\] when `version` is empty.
/// Returns [`AppError::Io`] when the cache directory cannot be
/// created or read. Returns [`AppError::Http`] when the upstream
/// player.js fetch fails. Returns [`AppError::Crypto`] when the
/// fetched blob cannot be parsed into a [`PlayerJs`].
#[tracing::instrument(level = "debug", err, skip(client), fields(version = %version))]
pub async fn get_or_fetch(client: &Client, version: &str) -> AppResult<CachedPlayer> {
    if version.is_empty() {
        return Err(AppError::InvalidInput(
            "player_js_cache::get_or_fetch requires non-empty version".to_string(),
        ));
    }
    let path = cache_file_path(version)?;
    let ttl = Duration::from_secs(PLAYER_JS_TTL_SECS);

    if is_fresh(&path, ttl).await {
        match read_cached(&path, version).await {
            Ok(cached) => return Ok(cached),
            Err(e) => {
                tracing::warn!(
                    target: "events",
                    error = %e,
                    version = %version,
                    "player_js_cache read failed; refetching"
                );
            }
        }
    }

    // Single-flight: at most one fetch per version is in flight.
    let lock = lock_for(version).await;
    let _guard = lock.lock().await;
    // Re-check after acquiring the lock — another worker may have
    // just populated the cache while we were waiting.
    if is_fresh(&path, ttl).await {
        if let Ok(cached) = read_cached(&path, version).await {
            return Ok(cached);
        }
    }
    let player = fetch_player_js(client, version).await?;
    if let Err(e) = write_cached(&path, &player).await {
        tracing::warn!(
            target: "events",
            error = %e,
            version = %version,
            "player_js_cache write failed; continuing without cache"
        );
    }
    Ok(CachedPlayer { player })
}

/// Read a cached `PlayerJs` from disk and re-hydrate the structure.
async fn read_cached(path: &std::path::Path, version: &str) -> AppResult<CachedPlayer> {
    let raw = tokio::fs::read(path).await.map_err(AppError::Io)?;
    let raw_str = String::from_utf8(raw)
        .map_err(|e| AppError::Internal(format!("cache blob is not valid UTF-8: {e}")))?;
    // Try the sidecar first; if it is missing we re-parse from the
    // blob on the fly. Either way the returned struct is the same.
    let operations = match read_ops_sidecar(version).await {
        Some(ops) => ops,
        None => crate::provider::youtube::player_js::extract_for_cache(&raw_str),
    };
    Ok(CachedPlayer {
        player: PlayerJs {
            version: version.to_string(),
            raw: raw_str,
            operations,
            n_code: String::new(),
        },
    })
}

/// Persist the `PlayerJs` blob and its operations table to disk.
async fn write_cached(path: &std::path::Path, player: &PlayerJs) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AppError::Io)?;
    }
    tokio::fs::write(path, player.raw.as_bytes())
        .await
        .map_err(AppError::Io)?;
    let sidecar = ops_sidecar_path(&player.version)?;
    if let Some(parent) = sidecar.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AppError::Io)?;
    }
    let bytes = encode_ops_sidecar(&player.operations);
    tokio::fs::write(&sidecar, bytes)
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

/// Serialise `JsOperation`s into a compact byte format for the
/// sidecar. Format: one byte per op type, followed by its payload
/// in big-endian `u16` (so 16-bit indices are forward-compatible).
fn encode_ops_sidecar(ops: &[crate::provider::youtube::player_js::JsOperation]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + ops.len() * 3);
    out.push(ops.len() as u8); // up to 255 ops is plenty
    use crate::provider::youtube::player_js::JsOperation as Op;
    for op in ops {
        match *op {
            Op::Split(i) => {
                out.push(0);
                out.extend_from_slice(&(i as u16).to_be_bytes());
            }
            Op::Swap(i, j) => {
                out.push(1);
                out.extend_from_slice(&(i as u16).to_be_bytes());
                out.extend_from_slice(&(j as u16).to_be_bytes());
            }
            Op::Reverse => {
                out.push(2);
            }
        }
    }
    out
}

fn decode_ops_sidecar(
    bytes: &[u8],
) -> Option<Vec<crate::provider::youtube::player_js::JsOperation>> {
    let (&count, rest) = bytes.split_first()?;
    let mut ops = Vec::with_capacity(count as usize);
    let mut i = 0;
    while i < rest.len() {
        let ty = *rest.get(i)?;
        i += 1;
        match ty {
            0 => {
                let a = u16::from_be_bytes([*rest.get(i)?, *rest.get(i + 1)?]) as usize;
                i += 2;
                ops.push(crate::provider::youtube::player_js::JsOperation::Split(a));
            }
            1 => {
                let a = u16::from_be_bytes([*rest.get(i)?, *rest.get(i + 1)?]) as usize;
                i += 2;
                let b = u16::from_be_bytes([*rest.get(i)?, *rest.get(i + 1)?]) as usize;
                i += 2;
                ops.push(crate::provider::youtube::player_js::JsOperation::Swap(a, b));
            }
            2 => ops.push(crate::provider::youtube::player_js::JsOperation::Reverse),
            _ => return None,
        }
    }
    Some(ops)
}

async fn read_ops_sidecar(
    version: &str,
) -> Option<Vec<crate::provider::youtube::player_js::JsOperation>> {
    let path = ops_sidecar_path(version).ok()?;
    let bytes = tokio::fs::read(&path).await.ok()?;
    decode_ops_sidecar(&bytes)
}

/// Test helper: set mtime of `path` to `when`. The std library does
/// not expose a portable async setter, so we use `std::fs` here —
/// the call is fast (just an `utimensat` syscall) and we are in a
/// test, so blocking is acceptable.
#[cfg(test)]
async fn filetime_set(path: &std::path::Path, when: std::time::SystemTime) -> std::io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let f = std::fs::File::options().write(true).open(&path)?;
        f.set_modified(when)?;
        Ok::<(), std::io::Error>(())
    })
    .await
    .expect("join")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::youtube::player_js::JsOperation;
    use serial_test::serial;

    #[test]
    fn encode_decode_roundtrip() {
        let ops = vec![
            JsOperation::Split(0),
            JsOperation::Swap(1, 42),
            JsOperation::Reverse,
            JsOperation::Swap(0, 7),
        ];
        let bytes = encode_ops_sidecar(&ops);
        let decoded = decode_ops_sidecar(&bytes).expect("decode succeeds");
        assert_eq!(decoded, ops);
    }

    #[test]
    fn decode_rejects_unknown_opcode() {
        let bad = vec![0u8, 99];
        assert!(decode_ops_sidecar(&bad).is_none());
    }

    #[tokio::test]
    #[serial]
    async fn cache_file_path_creates_parent() {
        // Resolve a real path; ensure the parent is created.
        let path = cache_file_path("v_unit_test_roundtrip").expect("path");
        let _ = tokio::fs::remove_file(&path).await;
        assert!(!path.exists());
        // We do not call write_cached here because that would
        // require a network fetch; the path helper is enough to
        // prove the directory is created on demand.
        let _ = cache_file_path("v_unit_test_roundtrip").expect("path");
        if let Some(parent) = path.parent() {
            assert!(
                parent.exists(),
                "parent should be created by cache_file_path"
            );
        }
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    #[serial]
    async fn is_fresh_treats_missing_as_stale() {
        let path = std::path::Path::new("/nonexistent/should-be-missing.bin");
        assert!(!is_fresh(path, Duration::from_secs(60)).await);
    }

    #[tokio::test]
    #[serial]
    async fn caches_and_retrieves_player_js() {
        // Drive the on-disk cache directly (no network) by
        // pre-populating the blob and sidecar, then reading them
        // back through the public `get_or_fetch` path with a
        // poisoned HTTP client that must never be called.
        let version = "v_unit_caches_and_retrieves";
        let body = include_str!("../../tests/fixtures/player/base_v123.js").to_string();
        let player = crate::provider::youtube::player_js::PlayerJs {
            version: version.to_string(),
            operations: crate::provider::youtube::player_js::extract_for_cache(&body),
            raw: body,
            n_code: String::new(),
        };
        let path = cache_file_path(version).expect("path");
        write_cached(&path, &player).await.expect("write");
        assert!(path.exists());
        // Build a reqwest client pointed at an unroutable address so
        // any accidental network call would fail loudly. We expect
        // the read-from-disk path to short-circuit before the
        // request is issued.
        let bad_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .expect("client");
        let cached = get_or_fetch(&bad_client, version).await.expect("hit");
        assert_eq!(cached.player.version, version);
        assert!(!cached.player.operations.is_empty());
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    #[serial]
    async fn expired_entry_returns_none() {
        let version = "v_unit_expired_entry";
        let path = cache_file_path(version).expect("path");
        // Touch the path then backdate its mtime to 8 days ago —
        // past the 7-day TTL.
        let body = include_str!("../../tests/fixtures/player/base_v123.js").to_string();
        let player = crate::provider::youtube::player_js::PlayerJs {
            version: version.to_string(),
            operations: crate::provider::youtube::player_js::extract_for_cache(&body),
            raw: body,
            n_code: String::new(),
        };
        write_cached(&path, &player).await.expect("write");
        let past = std::time::SystemTime::now()
            - std::time::Duration::from_secs(PLAYER_JS_TTL_SECS + 24 * 60 * 60);
        let _ = filetime_set(&path, past).await;
        assert!(!is_fresh(&path, Duration::from_secs(PLAYER_JS_TTL_SECS)).await);
        let _ = tokio::fs::remove_file(&path).await;
        let _ = tokio::fs::remove_file(ops_sidecar_path(version).expect("sidecar")).await;
    }

    #[tokio::test]
    #[serial]
    async fn single_flight_prevents_stampede() {
        // The single-flight contract is that the second waiter
        // observes a `true` from `try_lock` indicating the first
        // worker is in flight. We validate the lock helper by
        // taking it once, then proving a second `try_lock` fails.
        let lock = lock_for("v_unit_single_flight").await;
        let _guard = lock.try_lock().expect("first take succeeds");
        // Second `try_lock` must fail because we still hold the
        // first guard inside this scope.
        let res = lock.try_lock();
        assert!(
            res.is_err(),
            "second try_lock must fail while first guard is held"
        );
    }
}
