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
//! ferramenta baixa legendas do YouTube, sem nomear fontes.
//!
//! O `#![allow(missing_docs)]` no topo do arquivo e proposital:
//! este modulo e interno e nunca aparece em rustdoc publicado,
//! portanto nao precisa de doc comments em cada constante.
//!
//! Cada constante carrega `#[doc(hidden)]` para que `cargo public-api`
//! nao gere entradas com nomes sigilosos no baseline publicado.
//! O `Sigilo gate` no job CI `public-api` valida a invariante em
//! todo PR.

#[doc(hidden)]
pub(crate) const PROVIDER_A_PRIMARY_HOST: &str = "downsub.com";
#[doc(hidden)]
pub(crate) const PROVIDER_A_PRIMARY_PAGE: &str = "https://downsub.com/?url=";
#[doc(hidden)]
pub(crate) const PROVIDER_A_INFO_BASE: &str = "https://get-info.downsub.com/";
#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const PROVIDER_A_SUBTITLE_BASE: &str = "https://subtitle.downsub.com/";

#[doc(hidden)]
pub(crate) const PROVIDER_B_PRIMARY_HOST: &str = "www.downloadyoutubesubtitles.com";
#[doc(hidden)]
pub(crate) const PROVIDER_B_PRIMARY_PAGE: &str = "https://www.downloadyoutubesubtitles.com/?u=";
#[doc(hidden)]
pub(crate) const PROVIDER_B_API_PATH: &str = "/api.php";

#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const PROVIDER_B_REDIRECT_HOST: &str = "mywatchtones.com";

#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const OBFUSCATED_PASSWORD: &[u8] = b"Hnbdi4Nb6UYF3klv7Pma8Ze02jxt23us";

#[doc(hidden)]
pub(crate) const USER_AGENT_IDENTITY: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[doc(hidden)]
#[allow(dead_code)]
pub(crate) const COOKIE_ANTI_BOT_NAME: &str = "dysDedector";
