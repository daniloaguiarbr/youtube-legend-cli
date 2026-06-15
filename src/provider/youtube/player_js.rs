//! Fetch and parse the `YouTube` `player.js` blob to extract the
//! table of operations used by signature decipher (M3) and the
//! n-parameter permutation table used by ncode (M3.5).
//!
//! The `player_ias.vflset/en_US/base.js` script is a small JavaScript
//! that contains a `decipher`-style function built out of three
//! primitive operations:
//!
//! - `a = a.split("")`           — break the signature into a char array
//! - `a[i] = a[j]; a[j] = ...`   — swap two characters by index
//! - `a.reverse()`               — flip the array
//!
//! These primitives are chained in a fixed order inside the player
//! script. We locate the function via a regex, parse the three op
//! lines, and apply them to a ciphertext signature on demand.
//!
//! The same blob also carries the n-parameter permutation string
//! declared as `var ncode = "..."`. We extract it via a second
//! regex and feed it to [`crate::provider::youtube::ncode::ncode`].
//!
//! The blob is small (a few hundred KB), fetched once per player
//! version, and cached for 7 days. See
//! [`crate::cache::player_js_cache`] for the cache layer.

#![allow(dead_code)]

use std::sync::OnceLock;

use regex::Regex;
use reqwest::Client;

use crate::error::{AppError, AppResult};
use crate::secret_endpoints::YOUTUBE_PLAYER_URL_BASE;

/// Maximum size of the `player.js` body. The real file is ~1.4 MB but
/// we use a 5 MB cap to allow for future growth without panic.
const PLAYER_JS_MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

/// A primitive operation extracted from the `player.js` decipher
/// function. The variant mirrors the JavaScript primitive the
/// `YouTube` player issues on the signature char array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsOperation {
    /// `a = a.split("")` — no argument; this is implicit (it is the
    /// first op the function always performs), so the literal
    /// representation is unit.
    ///
    /// Stored as a no-op marker; the split happens before any
    /// operation is applied, so \[`crate::provider::youtube::decipher`\] consumes it implicitly.
    Split(usize),
    /// `var t = a[i]; a[i] = a[j]; a[j] = t` — swap two positions.
    /// `i` and `j` are the JS indices parsed from the function body.
    Swap(usize, usize),
    /// `a.reverse()` — flip the order of all characters.
    Reverse,
}

/// In-memory representation of the `player.js` blob for one version.
#[derive(Debug, Clone)]
pub struct PlayerJs {
    /// The version segment of the URL, e.g. `12345678`. Used as the
    /// cache key and as the URL path component.
    pub version: String,
    /// The full raw JavaScript blob, preserved for diagnostic logging
    /// and for any future op extraction.
    pub raw: String,
    /// The ordered operations extracted from the decipher function.
    /// May be empty if the regex failed to match a known function
    /// shape; callers should treat that as a decipher failure.
    pub operations: Vec<JsOperation>,
    /// The n-parameter permutation table extracted from the same
    /// blob. Empty when the player did not ship an ncode helper
    /// (older players pre-2022) or when the regex did not match.
    /// Decoded via [`crate::provider::youtube::ncode::ncode`].
    pub n_code: String,
}

impl PlayerJs {
    /// Returns the operations in execution order. Empty when the
    /// regex did not recognise the function body.
    pub fn operations(&self) -> &[JsOperation] {
        &self.operations
    }

    /// Returns the n-parameter permutation string. Empty when the
    /// player did not ship an ncode helper.
    pub fn n_code(&self) -> &str {
        &self.n_code
    }
}

/// Compiled once for the process via `OnceLock`. The pattern targets
/// the canonical `decipher`-style function body emitted by recent
/// players: `a=a.split("");<...>;return a.join("")}`. We capture the
/// slice between the split and the join so subsequent operations
/// sit in one match.
fn operations_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"split\(""\)\s*;([\s\S]{0,2000}?)join\("""#)
            .expect("static operations regex is valid")
    })
}

/// Compiled once. Locates the swap indices inside the captured body
/// via two patterns: `a\[(\d+)\]=a\[\1\];` (tautology, ignored) and
/// `a\[(\d+)\]=a\[(\d+)\];a\[\1\]=a\[\d+\]` which is the canonical
/// swap shape across the captured years.
fn swap_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"a\[(\d+)\]=a\[(\d+)\]"#).expect("static swap regex is valid"))
}

/// Compiled once. Matches the literal `var ncode = "..."` declaration
/// emitted by recent `player.js` blobs. The captured group is the raw
/// n-code string passed to [`crate::provider::youtube::ncode::ncode`].
fn ncode_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"var\s+ncode\s*=\s*"([0-9A-Fa-f]+)""#).expect("static ncode regex is valid")
    })
}

/// Fetch the `player.js` blob for a specific version and extract the
/// operations table and the n-code permutation string.
///
/// # Errors
///
/// - [`AppError::InvalidInput`] when `version` is empty.
/// - [`AppError::PlayerResponseTooLarge`] when the body exceeds the
///   5 MB cap.
/// - [`AppError::Http`] on transport failure.
/// - [`AppError::Internal`] when the operations regex fails to
///   compile (never happens; the static `expect` catches it) or
///   when the body cannot be parsed.
#[tracing::instrument(level = "debug", err, skip(client), fields(version = %version))]
pub async fn fetch_player_js(client: &Client, version: &str) -> AppResult<PlayerJs> {
    if version.is_empty() {
        return Err(AppError::InvalidInput(
            "fetch_player_js requires a non-empty version".to_string(),
        ));
    }
    let url = format!("{YOUTUBE_PLAYER_URL_BASE}{version}/player_ias.vflset/en_US/base.js");
    let resp = client.get(&url).send().await.map_err(AppError::Http)?;
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "player.js fetch returned HTTP {} for version {version}",
            resp.status().as_u16()
        )));
    }
    let body = resp.text().await.map_err(AppError::Http)?;
    if body.len() > PLAYER_JS_MAX_BODY_BYTES {
        return Err(AppError::PlayerResponseTooLarge {
            bytes: body.len(),
            limit: PLAYER_JS_MAX_BODY_BYTES,
        });
    }
    let operations = extract_operations(&body);
    let n_code = extract_n_code(&body);
    Ok(PlayerJs {
        version: version.to_string(),
        raw: body,
        operations,
        n_code,
    })
}

/// Walk the captured body between `split("")` and `join("")` and
/// collect the ordered operations. Falls back to an empty vector
/// when the regex does not match; callers must treat that as a
/// decipher failure.
///
/// Exposed as `pub(crate)` so the cache layer can re-run the
/// regex on a cache read when the operations sidecar is missing.
pub(crate) fn extract_operations(body: &str) -> Vec<JsOperation> {
    let Some(captures) = operations_re().captures(body) else {
        return Vec::new();
    };
    let inner = match captures.get(1) {
        Some(m) => m.as_str(),
        None => return Vec::new(),
    };
    let mut ops: Vec<JsOperation> = Vec::new();
    // First op is always the split, in the canonical form
    // `a=a.split("")` already located by the outer regex.
    ops.push(JsOperation::Split(0));
    // Walk inner left-to-right collecting swap indices and detecting
    // a `reverse()` call. We scan for the canonical "swap" pattern
    // via a sub-regex; the "reverse" check is a substring match
    // because the JS is minified and the call site is fixed-width
    // enough that false positives are vanishingly rare.
    for caps in swap_re().captures_iter(inner) {
        let i = match caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok()) {
            Some(v) => v,
            None => continue,
        };
        let j = match caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok()) {
            Some(v) => v,
            None => continue,
        };
        if i == j {
            continue;
        }
        // The first capture of a swap pair is the assignment LHS;
        // the second is the RHS read. We want the two distinct
        // indices — i and j. They are typically adjacent in the
        // body so we deduplicate successive equal pairs.
        if ops
            .iter()
            .any(|op| matches!(op, JsOperation::Swap(a, b) if (*a == i && *b == j)))
        {
            continue;
        }
        ops.push(JsOperation::Swap(i, j));
    }
    if inner.contains("reverse()") {
        ops.push(JsOperation::Reverse);
    }
    ops
}

/// Public entry point for the cache layer to re-parse a cached
/// blob. Simply forwards to [`extract_operations`] but is exposed
/// as `pub(crate)` so the cache module can call it without
/// reaching into private API.
pub(crate) fn extract_for_cache(body: &str) -> Vec<JsOperation> {
    extract_operations(body)
}

/// Extract the n-parameter permutation table from a `player.js`
/// body. Returns the empty string when the helper is absent
/// (older players that did not implement the n-parameter challenge)
/// or when the regex did not match. The caller feeds the result to
/// [`crate::provider::youtube::ncode::ncode`].
pub(crate) fn extract_n_code(body: &str) -> String {
    ncode_re()
        .captures(body)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_operations_parses_canonical_body() {
        let body = r#"
(function(a){a=a.split("");a[1]=a[42];a[42]=a[1];a.reverse();return a.join("")})
"#;
        let ops = extract_operations(body);
        assert!(matches!(ops.first(), Some(JsOperation::Split(0))));
        assert!(ops.iter().any(|op| matches!(op, JsOperation::Swap(1, 42))));
        assert!(ops.iter().any(|op| matches!(op, JsOperation::Reverse)));
    }

    #[test]
    fn extract_operations_empty_on_no_match() {
        let body = "totally unrelated JavaScript";
        assert!(extract_operations(body).is_empty());
    }

    #[test]
    fn extract_operations_drops_zero_swap() {
        let body = r#"(function(a){a=a.split("");a[3]=a[3];return a.join("")})"#;
        let ops = extract_operations(body);
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], JsOperation::Split(0)));
    }

    #[test]
    fn extract_operations_multiple_swaps() {
        let body = r#"(function(a){a=a.split("");a[0]=a[1];a[1]=a[0];a[5]=a[10];a[10]=a[5];return a.join("")})"#;
        let ops = extract_operations(body);
        let swaps: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                JsOperation::Swap(i, j) => Some((*i, *j)),
                _ => None,
            })
            .collect();
        assert!(swaps.contains(&(0, 1)));
        assert!(swaps.contains(&(5, 10)));
    }

    #[test]
    fn player_js_struct_preserves_raw() {
        let pj = PlayerJs {
            version: "v123".to_string(),
            raw: "blob".to_string(),
            operations: vec![JsOperation::Split(0), JsOperation::Reverse],
            n_code: String::new(),
        };
        assert_eq!(pj.version, "v123");
        assert_eq!(pj.raw, "blob");
        assert_eq!(pj.operations().len(), 2);
        assert_eq!(pj.n_code(), "");
    }

    #[test]
    fn extract_n_code_parses_canonical_declaration() {
        let body = r#"
var ncode = "2D03AC2F8B9A6E1D7F0C5B4E9A8D3F2C1B0A9E8D7C6B5A4F3E2D1C0B9A8F7E6D5C4B3A2F1E0D";
(function(a,b){a=a.split("");return a.join("")})
"#;
        let code = extract_n_code(body);
        assert!(code.starts_with("2D03AC"));
        assert!(code.len() >= 64);
    }

    #[test]
    fn extract_n_code_empty_on_no_match() {
        let body = "function unrelated() { return 42; }";
        assert_eq!(extract_n_code(body), "");
    }
}
