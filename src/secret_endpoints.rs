#![allow(missing_docs)]
#![allow(clippy::redundant_locals)]
//! Nomes de provedores, endpoints e tokens usados internamente pelo binário.
//!
//! ATENCAO: este modulo e INFORMACAO RESERVADA.
//! Os identificadores aqui NAO DEVEM ser expostos em
//! - documentacao publica (README, CHANGELOG)
//! - mensagens de stderr
//! - logs de tracing
//! - issues de bug tracker publico
//! - comentarios de codigo fora deste modulo
//!
//! Politica de governanca: a existencia deste pipeline de 2 provedores
//! e segredo de implementacao. O README publico afirma apenas que a
//! ferramenta baixa legendas do `YouTube`, sem nomear fontes.
//!
//! O `#![allow(missing_docs)]` no topo do arquivo e proposital:
//! este modulo e interno e nunca aparece em rustdoc publicado,
//! portanto nao precisa de doc comments em cada constante.
//!
//! Cada constante carrega `#[doc(hidden)]` para que `cargo public-api`
//! nao gere entradas com nomes sigilosos no baseline publicado.
//! O `Sigilo gate` no job CI `public-api` valida a invariante em
//! todo PR.

// GAP-AUD-2026-038: noteey.com is the exclusive provider since v0.3.2.
// The noteey homepage hosts a vanilla Vue form (URL input + Get
// Subtitle button + transcript pane). Headless extraction is the
// user-mandated path; a parallel JSON API exists but is not used by
// this provider (see GAP-AUD-2026-043 in gaps.md for the future HTTP
// shortcut).
//
// The constants are `#[allow(dead_code)]` so the `snapshot` binary
// (which builds without the `headless` feature) compiles cleanly.
#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const NOTEEY_PRIMARY_HOST: &str = "www.noteey.com";
#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const NOTEEY_PRIMARY_PAGE: &str = "https://www.noteey.com/youtube-subtitle-downloader";
