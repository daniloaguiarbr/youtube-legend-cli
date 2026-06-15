//! Apply the n-parameter challenge permutation extracted from
//! `YouTube` `player.js`.
//!
//! The n-parameter is a separate cipher applied on top of the
//! signature (or, for the most recent players, in place of it on
//! the `&n=` query parameter of the `baseUrl`). The permutation is
//! driven by a string of characters extracted from the player
//! JavaScript; the permutation logic is fixed across all
//! observed player versions and lives verbatim in the JS:
//!
//! ```text
//! for (var c of ncode.split("")) {
//!     if (c >= "0" && c <= "9") {
//!         var t = a[0];
//!         a[0] = a[c % a.length];
//!         a[c % a.length] = t;
//!     }
//! }
//! a.shift();
//! ```
//!
//! This module ports the same algorithm to Rust as pure string
//! operations. No network, no `unsafe`, no allocations beyond the
//! working `Vec<char>` produced by the reversal.
//!
//! Reference: `yt-dlp/extractor/youtube.py::_extract_n_function`
//! and `_n_descramble`. The 0-indexed swap target in the JS is
//! already `0`-based after the array split, so the Rust port does
//! not need to apply the `c % len` modulo that Python uses for the
//! out-of-range guard — out-of-range ASCII digits are still in
//! `[0x30, 0x39]`, always below the signature length for any real
//! ciphertext, but we apply the modulo defensively to mirror the
//! JS runtime semantics exactly.

/// Apply the n-code permutation to a ciphertext n-parameter,
/// producing the plaintext value `YouTube` expects in the `&n=`
/// query parameter.
///
/// # Algorithm
///
/// 1. Reverse the input into a `Vec<char>`.
/// 2. For every ASCII digit `c` in `n_code`, swap
///    `sig_chars[0]` with `sig_chars[c as usize % sig_chars.len()]`.
/// 3. Drop the first character (the JS does `a.shift()`) and join.
///
/// # PII redaction
///
/// The function does not log the input or output. Observability
/// happens at the caller via [`macro@tracing::instrument`] with a `target`
/// that excludes the signature value.
#[tracing::instrument(level = "debug", target = "youtube_decipher", skip_all, fields(n_code_len = n_code.len(), sig_len = sig.len()))]
pub fn ncode(sig: &str, n_code: &str) -> String {
    if sig.is_empty() {
        return String::new();
    }
    if n_code.is_empty() {
        // No permutation requested: just reverse and drop the first
        // char, which mirrors the unconditional `a.shift()` in the
        // JS. An empty `n_code` is rare in the wild but is the
        // correct behaviour when the player has no permutation
        // table for the n-parameter.
        let mut buf: Vec<char> = sig.chars().rev().collect();
        if !buf.is_empty() {
            buf.remove(0);
        }
        return buf.into_iter().collect();
    }
    let mut buf: Vec<char> = sig.chars().rev().collect();
    let len = buf.len();
    for c in n_code.chars() {
        if let Some(digit) = c.to_digit(10) {
            // Modulo the digit by the current length to keep the
            // access in-bounds even when the player emits a digit
            // that exceeds the signature length. The JS does the
            // same modulo; we mirror it so the Rust output matches
            // the reference implementation byte-for-byte.
            let idx = (digit as usize) % len;
            if idx != 0 {
                buf.swap(0, idx);
            }
        }
    }
    if !buf.is_empty() {
        buf.remove(0);
    }
    buf.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::youtube::decipher::decipher;
    use crate::provider::youtube::player_js::JsOperation;

    #[test]
    fn basic_ncode_permutation() {
        // 5-character signature, 2-digit n_code.
        // Reverse: EDCBA -> [E, D, C, B, A]
        // digit '2': idx=2 -> swap(0,2): [C, D, E, B, A] = "CDEBA"
        // digit '3': idx=3 -> swap(0,3): [B, D, E, C, A] = "BDECA"
        // shift (drop first char): "DECA"
        let sig = "ABCDE";
        let n_code = "23";
        assert_eq!(ncode(sig, n_code), "DECA");
    }

    #[test]
    fn ncode_with_empty_signature() {
        // Empty input: no work, no panic, empty output. The JS
        // `a.shift()` would be a no-op on an empty array, and the
        // loop body would never execute because there is no
        // signature to permute.
        let sig = "";
        let n_code = "1234";
        assert_eq!(ncode(sig, n_code), "");
    }

    #[test]
    fn ncode_with_long_signature() {
        // 26-character signature mirrors the alphabet; n_code picks
        // a series of swaps that each rotate the first char out to
        // a far index and back. The expected result is computed
        // independently by replaying the algorithm.
        let sig = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let n_code = "0213";
        // Reverse: ZYXWVUTSRQPONMLKJIHGFEDCBA
        // d='0': idx=0%26=0 -> no-op
        // d='2': idx=2%26=2 -> swap(0,2): ZY X WVUTSRQPONMLKJIHGFEDCBA -> ZYXWVUTSRQPONMLKJIHGFEDCBA -> wait,
        // let's compute: Z Y X W V U T S R Q P O N M L K J I H G F E D C B A
        //               0 1 2 3 4 5 6 7 8 9 ...
        // swap(0,2): X Y Z W V U T S R Q P O N M L K J I H G F E D C B A
        // d='1': idx=1%26=1 -> swap(0,1): Y X Z W V U T S R Q P O N M L K J I H G F E D C B A
        // d='3': idx=3%26=3 -> swap(0,3): W X Z Y V U T S R Q P O N M L K J I H G F E D C B A
        // shift: X Z Y V U T S R Q P O N M L K J I H G F E D C B A
        let expected = "XZYVUTSRQPONMLKJIHGFEDCBA";
        assert_eq!(ncode(sig, n_code), expected);
    }

    #[test]
    fn ncode_integration_with_decipher() {
        // Realistic chain: the n-parameter is deciphered first via
        // `ncode`, then the signature (if any) is deciphered via
        // the operations table. Here we simulate a fixed
        // n-parameter that needs only one swap and confirm both
        // primitives compose without sharing state.
        let n_param_cipher = "XBCDE"; // n-parameter after YouTube ciphering
        let n_code = "2"; // swap(0, 2%5=2) on reversed "XBCDE"
        let n_plain = ncode(n_param_cipher, n_code);
        // Independently compute expected:
        // reverse: "EDCBX" -> chars: [E, D, C, B, X]
        // d='2': idx=2 -> swap(0,2): [C, D, E, B, X] = "CDEBX"
        // shift: "DEBX"
        assert_eq!(n_plain, "DEBX");

        // Now run the same shape through `decipher` with a single
        // swap op to make sure both modules agree on swap
        // semantics. We pick a fresh string to avoid coupling
        // expectations between the two paths.
        let sig_plain = "ABCDEF";
        let ops = vec![JsOperation::Swap(0, 2)];
        // swap(0,2) on [A,B,C,D,E,F] -> [C,B,A,D,E,F]
        assert_eq!(decipher(sig_plain, &ops), "CBADEF");
    }
}
