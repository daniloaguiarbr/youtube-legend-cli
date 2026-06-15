//! Apply the operations table extracted from `player.js` to a
//! `YouTube` signature cipher text.
//!
//! The algorithm is the same one used by `yt-dlp` and
//! `nuclearplayer/ytdl-core` (Rust fork): start with the signature
//! broken into a `Vec<char>`, then apply each operation in order
//! until the array is reassembled with `join("")`.
//!
//! All operations are pure string manipulations — no network, no
//! allocations beyond the working `Vec<char>`. The module is
//! allocation-conscious because decipher is on the hot path of
//! every signature-bearing `baseUrl`.

use crate::provider::youtube::player_js::JsOperation;

/// Apply the operations table to a ciphertext signature, producing
/// the plaintext signature that `YouTube` expects in the
/// `&sig=<plaintext>` query parameter.
///
/// # Returns
///
/// The deciphered signature as a `String`. When `ops` is empty the
/// input is returned unchanged — this matches the legacy
/// `&sig=<ciphertext>` path that bypassed `player.js` entirely
/// (e.g. very old players that did not cipher at all).
///
/// # PII redaction
///
/// The function does not log the input or output; observability is
/// handled by callers via [`macro@tracing::instrument`] with a `target`
/// that excludes the signature value.
#[tracing::instrument(level = "debug", target = "youtube_decipher", skip_all, fields(op_count = ops.len(), sig_len = sig.len()))]
pub fn decipher(sig: &str, ops: &[JsOperation]) -> String {
    if ops.is_empty() {
        return sig.to_string();
    }
    let mut buf: Vec<char> = sig.chars().collect();
    for op in ops {
        match *op {
            JsOperation::Split(_) => {
                // Split is a no-op at runtime: the caller has
                // already turned the string into a Vec<char> above.
                // We still iterate the variant so the order
                // matches what the JS function does, but the
                // primitive is implicit in the data layout.
            }
            JsOperation::Swap(i, j) => {
                // Bounds check: out-of-range swaps are tolerated by
                // the JS runtime (they become no-ops on the
                // undefined access) so we mirror that to avoid
                // panicking on a malformed table.
                if let (Some(a), Some(b)) = (buf.get(i), buf.get(j)) {
                    if i != j {
                        let av = *a;
                        let bv = *b;
                        if let Some(slot_a) = buf.get_mut(i) {
                            *slot_a = bv;
                        }
                        if let Some(slot_b) = buf.get_mut(j) {
                            *slot_b = av;
                        }
                    }
                }
            }
            JsOperation::Reverse => {
                buf.reverse();
            }
        }
    }
    buf.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::youtube::player_js::JsOperation;

    #[test]
    fn apply_split_operation() {
        // Split is a no-op at the `Vec<char>` level; the sig is
        // already a char vector after `chars().collect()`. Empty
        // ops should be a passthrough.
        let sig = "ABCDEFG";
        let ops: Vec<JsOperation> = vec![JsOperation::Split(0)];
        assert_eq!(decipher(sig, &ops), "ABCDEFG");
    }

    #[test]
    fn apply_swap_operation() {
        let sig = "ABCDE";
        let ops = vec![JsOperation::Swap(0, 1)];
        assert_eq!(decipher(sig, &ops), "BACDE");
    }

    #[test]
    fn apply_reverse_operation() {
        let sig = "ABCDE";
        let ops = vec![JsOperation::Reverse];
        assert_eq!(decipher(sig, &ops), "EDCBA");
    }

    #[test]
    fn combined_operations() {
        // Mimic the canonical ytdl-core table:
        //   split, swap(0, 41), reverse.
        // Input "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij"
        // split index 41 -> 'j' (index 41 of the 50-char string).
        // After swap(0, 41) the first char becomes the 42nd and
        // vice versa. Reverse flips the entire array.
        let sig = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij";
        let ops = vec![
            JsOperation::Split(0),
            JsOperation::Swap(0, 41),
            JsOperation::Reverse,
        ];
        let out = decipher(sig, &ops);
        // Independently compute the expected result.
        let mut chars: Vec<char> = sig.chars().collect();
        chars.swap(0, 41);
        chars.reverse();
        let expected: String = chars.into_iter().collect();
        assert_eq!(out, expected);
        assert_ne!(out, sig, "operations must change the value");
    }

    #[test]
    fn empty_ops_is_passthrough() {
        let sig = "ABCDEF";
        let ops: Vec<JsOperation> = vec![];
        assert_eq!(decipher(sig, &ops), sig);
    }

    #[test]
    fn out_of_range_swap_is_tolerated() {
        let sig = "ABC";
        let ops = vec![JsOperation::Swap(10, 11)];
        // Out-of-range indices are no-ops; the function must not
        // panic on a malformed table.
        assert_eq!(decipher(sig, &ops), "ABC");
    }

    #[test]
    fn swap_same_index_is_tolerated() {
        let sig = "ABCDE";
        let ops = vec![JsOperation::Swap(2, 2)];
        assert_eq!(decipher(sig, &ops), "ABCDE");
    }

    #[test]
    fn multiple_swaps_compose() {
        let sig = "ABCDEF";
        let ops = vec![JsOperation::Swap(0, 1), JsOperation::Swap(4, 5)];
        // After (0,1): "BACDEF"
        // After (4,5): "BACDFE"
        assert_eq!(decipher(sig, &ops), "BACDFE");
    }

    #[test]
    fn unicode_chars_preserved() {
        // 7-char unicode string. Swap(0, 6) swaps the first and
        // last codepoints.
        let sig = "caf\u{00e9}na\u{00ef}"; // 7 chars: c a f é n a ï
        let ops = vec![JsOperation::Swap(0, 6)];
        let chars: Vec<char> = sig.chars().collect();
        let expected: String = {
            let mut v = chars;
            v.swap(0, 6);
            v.into_iter().collect()
        };
        assert_eq!(decipher(sig, &ops), expected);
    }
}
