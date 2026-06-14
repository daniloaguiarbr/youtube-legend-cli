//! Unicode NFC normalisation helpers.
//!
//! This module is `pub(crate)` but the doc-tests below exercise the
//! public `parse::srt_to_text` helper, which internally calls
//! `normalize_nfc`. The lint that normally denies doc-tests in
//! private modules is suppressed for that reason.
#![allow(rustdoc::private_doc_tests)]

use unicode_normalization::UnicodeNormalization;

/// Return the Unicode NFC form of `input`.
///
/// # Examples
///
/// ```
/// use youtube_legend_cli::parse::srt_to_text;
///
/// let srt = "1\n00:00:01,000 --> 00:00:02,000\nOlá mundo\n";
/// let text = srt_to_text(srt).unwrap();
/// assert!(text.contains("Olá"));
/// ```
pub(crate) fn normalize_nfc(input: &str) -> String {
    input.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_unchanged() {
        assert_eq!(normalize_nfc("hello world"), "hello world");
    }

    #[test]
    fn accented_nfc_canonical() {
        let nfc = normalize_nfc("Olá");
        assert_eq!(nfc, "Olá");
    }

    #[test]
    fn nfd_input_gets_canonicalized() {
        let nfd = "Ola\u{0301}";
        let nfc = normalize_nfc(nfd);
        assert_eq!(nfc, "Olá");
    }

    #[test]
    fn japanese_katakana_nfc() {
        let original = "コンニチハ";
        let nfc = normalize_nfc(original);
        assert_eq!(nfc, "コンニチハ");
    }

    #[test]
    fn emoji_nfc_unchanged() {
        let s = "Hello 👋 World 🌍";
        let nfc = normalize_nfc(s);
        assert!(nfc.contains("👋"));
        assert!(nfc.contains("🌍"));
    }

    #[test]
    fn empty_string_unchanged() {
        assert_eq!(normalize_nfc(""), "");
    }
}
