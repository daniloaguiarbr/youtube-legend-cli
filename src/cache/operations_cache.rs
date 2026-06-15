//! Lightweight in-memory cache for parsed operations tables.
//!
//! The table of `JsOperation`s extracted from `player.js` is
//! significantly smaller than the raw blob (a few bytes vs ~1.4 MB),
//! and the regex parsing is the slow part of M3. We therefore
//! cache the operations separately so even cold disk reads of
//! the player blob do not have to re-run the regex.
//!
//! Concurrency: a `parking_lot::Mutex` (re-exported as the standard
//! `std::sync::Mutex` to avoid pulling in a new dependency) guards
//! the `HashMap`. Reads are O(1) and very hot on the provider chain
//! path. The cache is process-local and never written to disk —
//! operations tables are versioned by the player URL segment, and
//! the sidecar in [`crate::cache::player_js_cache`] already
//! persists them across process restarts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::AppResult;
use crate::provider::youtube::player_js::JsOperation;

/// Process-wide map of `version -> operations`. Lazily allocated on
/// first access; the `OnceLock` makes the allocation cost
/// `O(1)` and free of races during cold start.
fn table() -> &'static Mutex<HashMap<String, Arc<Vec<JsOperation>>>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Arc<Vec<JsOperation>>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Look up a cached operations table by version. Returns a clone of
/// the `Arc` so callers can hold the result outside the mutex.
///
/// # Panics
///
/// Panics if the underlying [`std::sync::Mutex`] is poisoned (i.e.
/// another thread that held the mutex panicked while holding it).
pub fn get(version: &str) -> Option<Vec<JsOperation>> {
    let guard = table().lock().expect("operations_cache mutex poisoned");
    guard.get(version).map(|arc| (**arc).clone())
}

/// Insert (or replace) the operations table for a version. The
/// vector is wrapped in `Arc` to keep clones cheap on hot paths.
///
/// # Errors
///
/// Returns \[`crate::error::AppError::InvalidInput`\] when `version` is empty.
///
/// # Panics
///
/// Panics if the underlying [`std::sync::Mutex`] is poisoned (i.e.
/// another thread that held the mutex panicked while holding it).
pub fn put(version: &str, ops: &[JsOperation]) -> AppResult<()> {
    if version.is_empty() {
        return Err(crate::error::AppError::InvalidInput(
            "operations_cache::put requires non-empty version".to_string(),
        ));
    }
    let mut guard = table().lock().expect("operations_cache mutex poisoned");
    guard.insert(version.to_string(), Arc::new(ops.to_vec()));
    Ok(())
}

/// Wipe a single entry. Mostly useful in tests; the production
/// path never invalidates a single key.
///
/// # Panics
///
/// Panics if the underlying [`std::sync::Mutex`] is poisoned (i.e.
/// another thread that held the mutex panicked while holding it).
pub fn invalidate(version: &str) {
    let mut guard = table().lock().expect("operations_cache mutex poisoned");
    guard.remove(version);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_then_get_roundtrip() {
        invalidate("v_roundtrip");
        let ops = vec![JsOperation::Split(0), JsOperation::Reverse];
        put("v_roundtrip", &ops).expect("put succeeds");
        let got = get("v_roundtrip").expect("present");
        assert_eq!(got, ops);
        invalidate("v_roundtrip");
    }

    #[test]
    fn get_returns_none_for_missing_version() {
        assert!(get("v_definitely_does_not_exist_xyz").is_none());
    }

    #[test]
    fn invalidate_removes_entry() {
        let ops = vec![JsOperation::Swap(1, 2)];
        put("v_to_invalidate", &ops).expect("put");
        assert!(get("v_to_invalidate").is_some());
        invalidate("v_to_invalidate");
        assert!(get("v_to_invalidate").is_none());
    }

    #[test]
    fn put_rejects_empty_version() {
        let res = put("", &[JsOperation::Reverse]);
        assert!(matches!(res, Err(crate::error::AppError::InvalidInput(_))));
    }

    #[test]
    fn put_replaces_existing_entry() {
        invalidate("v_replace");
        put("v_replace", &[JsOperation::Reverse]).expect("put");
        put("v_replace", &[JsOperation::Split(0)]).expect("replace");
        let got = get("v_replace").expect("present");
        assert_eq!(got, vec![JsOperation::Split(0)]);
        invalidate("v_replace");
    }
}
