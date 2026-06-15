# Changelog

[English](CHANGELOG.md) | [Português Brasileiro](CHANGELOG.pt-BR.md)

## [Unreleased]

### Adicionado
- ProviderYouTubeDirect (`src/provider/provider_youtube_direct.rs`) — GAP-001 M4 — provedor nativo do YouTube que consulta o endpoint público `ytInitialPlayerResponse` e `captionTracks[].baseUrl` sem depender de serviços third-party
- Módulo `src/provider/youtube/` com:
  - `player_response.rs` (M1): parser do `ytInitialPlayerResponse` extraído da watch page via regex
  - `player_js.rs` e `decipher.rs` (M3): signature decipher portada de `base.js` com cache XDG
  - `ncode.rs` (M3.5): permutação do parâmetro n para vídeos protegidos
  - `caption_track.rs`: tipo de domínio para tracks de legenda
- Parser Srv3/Json3 em `src/parse/srv3.rs` (M2): converte formatos nativos do YouTube para SRT
- Binário `youtube-direct-probe` em `src/bin/` para diagnóstico
- Fixtures de teste: `tests/fixtures/player/base_v123.js`, `tests/fixtures/player/ncode_v456.js`, `tests/fixtures/timedtext/sample_{en.srv3,multiline.srv3,pt.json3}`
- Erros novos em `src/error.rs`: `SignatureDecipherFailed`, `PlayerResponseMissing`, `CaptionTrackNotFound`, `TimedtextUpstreamError`
- 196+ testes verdes (incremento de ~30 testes do GAP-001)

### Alterado
- Refatoração: `src/cache.rs` virou `src/cache/` (mod operations_cache, player_js_cache) — M3 do GAP-001 introduziu cache XDG para o `base.js` do player
- Chain de providers reordenada: `Auto=ProviderYouTubeDirect → ProviderA → ProviderB → [ProviderHeadless]`

### Corrigido
- META-GAP-B (a fazer): proteção DoS em `player_response.rs` — `serde_json` sem `arbitrary_limit` (ver `gaps.md` META-GAP-B)

## [0.3.0-rc.local] - 2026-06-15

NÃO PUBLICADO. Aguardando aprovação do usuário para tag v0.3.0 e push.

### Adicionado
- ProviderYouTubeDirect com parser de ytInitialPlayerResponse
- Conversor Srv3 e JSON3 para SRT
- Signature decipher via tabela do player.js
- n-parameter decipher via função ncode
- PlayerJsCache (XDG, TTL 7 dias, single-flight)
- OperationsCache (separado do player.js)
- Flags --provider, --asr, --no-fallback
- bin/youtube-direct-probe (ferramenta standalone de debug)
- Schema JSON docs/schemas/caption-track.schema.json
- Gate CI .github/workflows/youtube-direct.yml

### Estatísticas
- 167 testes passando (0 falhando)
- Provider trait mantida compatível


Todas as mudanças notáveis neste projeto são documentadas neste arquivo.

O formato segue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
e este projeto adere ao [Semantic Versioning](https://semver.org/).

## [0.2.8] - 2026-06-14

### Fixed
- `rust-version` em `Cargo.toml` declarava `1.96.0` enquanto o
  código compila e testa limpo em `1.88.0`. Reduzido para
  `1.88.0` para que usuários em stable Fedora (rustc 1.94.1) e o
  ecossistema 1.88+ mais amplo possam rodar
  `cargo install youtube-legend-cli` sem o cliente cargo recusar
  o manifesto. O `rust-toolchain.toml` local permanece na latest
  stable (1.96.0) para desenvolvimento reproduzível; o contrato
  com usuários finais vive em `Cargo.toml` `rust-version`.

## [0.2.7] - 2026-06-14

### Fixed
- Metadata do crates.io: o slug de categoria `web-programming::scraping`
  foi depreciado; reduzido para apenas `["command-line-utilities"]`.

## [0.2.6] - 2026-06-14

### Added
- Sete novas flags globais de CLI: `--config <PATH>`,
  `--log-level <LEVEL>`, `--log-format <FORMAT>`, `--color <WHEN>`,
  `--no-progress`, `--dry-run`, `--yes` (resolve lacunas
  ergonômicas para scripting, uso em daemon e ingestão de logs).
  Veja `src/cli.rs` e `src/logging.rs`.
- `mimalloc` como o alocador global em `src/main.rs` para reduzir
  overhead de alocação no hot path de busca de legendas (buffers
  de corpo HTTP, parsing de URL).
- Alvo de benchmark baseado em Criterion via
  `cargo bench --bench cache_bench`. Três micro-benchmarks cobrem
  o compositor de chave de cache, a checagem de comprimento de
  URL e o parser BCP 47. Execute com
  `cargo bench --bench cache_bench`; o CI verifica a compilação
  do alvo via `cargo bench --no-run`.
- Feature Cargo `headless` gateando um provedor opcional de
  fallback headless-browser (`src/provider/provider_headless.rs`).
  Dirige uma instância local de Chromium/Chrome via `chromiumoxide`
  para rodar o próprio JavaScript do site upstream e baixar a
  legenda pela sessão same-origin da página, recuperando downloads
  quando os provedores HTTP puros são bloqueados por Cloudflare ou
  endpoints browser-gated. Desabilitada por padrão; habilite com
  `cargo build --features headless`. Resolve o browser via `$CHROME`
  ou paths de instalação conhecidos.
- Conformidade com `robots.txt` para ambos os provedores
  (NFR-007, GAP-010). Os fetchers consultam
  `src/provider/robots.rs` antes de emitir qualquer requisição ao
  host upstream; o caminho `Disallow` é tratado como
  `EX_UNAVAILABLE` para que o tooling downstream possa ramificar
  no mesmo exit code de uma falha de rede.
- Teste de integração offline-cache `tests/integration/offline_cache.rs`
  (NFR-005, GAP-008) exercitando o round-trip
  `(read cache, no network, plain text output)` com uma URL de
  fixture.
- `tests/integration/rss.rs` aplicando o orçamento NFR-002 de RSS
  de 100 MiB durante runs de integração.
- `tests/integration/provider_a_wiremock.rs` e
  `tests/integration/provider_b_wiremock.rs` exercitando ambos os
  provedores contra mocks `wiremock` para que o binário possa ser
  testado sem nunca tocar no upstream ao vivo (GAP-012).
- `tests/integration/signal_handler_stress.rs` (gateado
  `#[ignore]`) para comportamento de `SIGINT` / `SIGTERM` sob
  stress. Apenas execução local.
- `AppError::RateLimited` para respostas HTTP 429 upstream,
  carregando o delta-seconds do `Retry-After` parseado; a camada
  de retry o honra com fallback de 60 s limitado a 300 s
  (EC-021). Um erro de rate limit de um provedor é preservado
  através da cadeia mesmo quando um provedor posterior falha
  genericamente.
- Cada tentativa de retry é logada como um evento estruturado
  tracing `event = "retry"` com número da tentativa e próximo
  delay (FR-013).
- `Retry-After` agora também é honrado em formato RFC 2822
  HTTP-date, convertido para delta-seconds contra o clock atual
  e grampeado em zero quando a data está no passado (EC-021,
  clock-skew safe).
- `#[tracing::instrument]` em 14 entry points da API pública
  interna (GAP-011) — `commands::run`, `extract::run`,
  `batch::run`, `ProviderA::fetch_subtitle`,
  `ProviderB::fetch_subtitle`, `ProviderChain::fetch_subtitle`,
  `cache::{read,write,path}`, `retry::retry_with_backoff`,
  `parse::{extract_video_id,srt_to_text}`,
  `io::read_url_from_stdin`, `commands::batch::dedup_set` e o novo
  `provider::robots::check`.
- Extrator heurístico-fallback para drift de HTML do provider-A
  (EC-024, GAP-023): quando o seletor primário `scraper::Html`
  falha, o fetcher agora também percorre o bloco
  `JSON-LD VideoObject` na página em busca de URLs de legenda.
- Matriz de cross-compile agora cobre seis alvos via job
  `cross-compile` do `ci.yml` (GAP-022, GAP-024):
  `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-musl`, `x86_64-pc-windows-msvc`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin` (os dois últimos
  com `continue-on-error: true` porque requerem `osxcross`).
- `tests/integration/io.rs` cobre `read_url_from_stdin` e
  `read_urls_from_stdin` (GAP-026) para as três formas de
  entrada (URL única, batch via `--batch`, uma-URL-por-linha).
- Binário companheiro `src/bin/snapshot.rs`
  (`cargo run --bin snapshot`) sonda ambos os provedores e grava
  snapshots HTML redacted sob `tests/fixtures/snapshots/<date>/`
  para detecção de drift. O módulo `secret_endpoints` é consumido
  via `#[path = "../secret_endpoints.rs"] mod secret_endpoints;`
  para que a API pública do crate nunca reexporte as constantes
  de host.
- `docs/agent-teams-workflow.md` — playbook para o fluxo de Agent
  Teams (plan / spawn / validate / cleanup), com os quatro modos
  conhecidos de quebra registrados conforme o fluxo amadurece.
- `docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md`
  — ADR-0009 registra a regra de que `Cargo.toml` é de
  propriedade de exatamente uma tarefa por sessão sob o fluxo de
  Agent Teams, após o incidente de 2026-06-13 de drift
  mimalloc/Cargo.toml.
- `--lang` agora aceita tags de idioma BCP 47
  (`pt-BR`, `pt_BR.UTF-8`, `EN-us`); locales malformados ou
  não suportados são rejeitados com exit code `2` (FR-009).
- `[package.metadata.docs.rs]` `targets` agora inclui tanto
  `x86_64-apple-darwin` quanto `aarch64-apple-darwin`
  (era 4 alvos, agora 6).
- rustdoc de crate, docs `//!` de módulo e docs `///` de item ao
  longo da API pública.
- `cargo doc --no-deps --all-features` e `cargo test --doc`
  para o CI.
- `cargo-deny`, `cargo-audit`, `cargo-public-api` (com gate
  sigilo), `cargo-semver-checks`, `lychee`, e os jobs matrix-os
  e nightly de Agent-Teams no CI.
- `LICENSE` (dual MIT / Apache-2.0), `CONTRIBUTING.md`,
  `SECURITY.md`, e `CODE_OF_CONDUCT.md`.
- `clippy.toml` com 3 métodos desautorizados,
  `cognitive-complexity-threshold = 30`,
  `too-many-arguments-threshold = 8` (GAP-019).
- `rustfmt.toml`, `rust-toolchain.toml`.
- `[badges.maintenance]` em `Cargo.toml`.
- Três exemplos executáveis em `examples/`: `single_url`, `batch`
  e `json_output`.
- `#[non_exhaustive]` em cada enum público.

### Fixed
- A requisição da API do Provider-B era construída a partir de
  uma constante de host sem scheme, então o `reqwest` a rejeitava
  no send com um erro opaco do builder e o POST nunca saía do
  processo; a URL agora é construída com um scheme `https://`
  explícito. Um teste de regressão afirma que a URL da API parseia
  como URL HTTPS absoluta.
- `default-run` agora está setado para que `cargo run` e o
  harness do corpus de integração resolvam o binário CLI sem
  ambiguidade.
- Path do endpoint AJAX do Provider-B corrigido após o provedor
  renomeá-lo; o path anterior retornava HTTP 404.
- Provider-B agora descobre seu path de endpoint AJAX a partir
  do JavaScript inline da página em runtime, com o path
  compilado como fallback, para que o cliente se adapte
  automaticamente quando o provedor renomeia o endpoint.
- **BREAKING**: exit codes migrados do esquema legado 2-7 para
  BSD `sysexits.h` (64-78). Mapeamento: uso/entrada inválida
  `2` → `64` (`EX_USAGE`); URL inválida `3` → `65` (`EX_DATAERR`);
  sem legenda `4` → `66` (`EX_NOINPUT`); todos os provedores
  indisponíveis ou rate limited `5` → `69` (`EX_UNAVAILABLE`);
  timeout HTTP `6` → `70` (`EX_SOFTWARE`); I/O, HTTP, serde,
  crypto, subtitle-too-large, e erros internos `7` → `70`
  (`EX_SOFTWARE`). Pipelines que chaveavam no código legado
  exato devem atualizar seus branches. Veja
  `src/error.rs::sysexits` e a tabela de exit codes do README
  para a referência canônica.
- `secret_endpoints` estava exposto como `pub mod` no crate root
  público, vazando os hostnames upstream via rustdoc; o módulo
  agora é `pub(crate) mod secret_endpoints;` e o binário
  `snapshot` o consome via `#[path = "../secret_endpoints.rs"]`
  (GAP-007). Um gate de CI (job `public-api` do `ci.yml`) falha
  o build se `pub mod secret_endpoints` regredir.
- Superfície de `pub use` no crate root público reduzida de 14+
  símbolos para 2 reexports justificados
  (`Cli`, `FormatArg`, `LanguageArg`, `AppError`, `AppResult`,
  `NoSubtitleReason`) (GAP-017).
- Módulo `text` agora é `pub(crate) mod text` em vez de
  `pub mod text` já que normalização Unicode NFC é um helper
  interno (GAP-018, GAP-027).
- Lista de excludes do `cargo.toml` estendida para cobrir
  `docs_prd/**`, `docs_rules/**`, `.github/**`,
  `tests/fixtures/snapshots/**`, `*.bak.*`, `*.tar.gz`. Previne
  que artefatos de build e arquivos pessoais de trabalho sejam
  empacotados no crate publicado.
- Comentários `// SAFETY:` adicionados a todos os blocos `unsafe`
  em `cache.rs`.
- `pub use` nos módulos `cli` e `error` re-assertado; removido
  o reexport órfão de `text`.
- Arquivos `src/*.bak.*` obsoletos removidos da árvore de
  trabalho; apenas `tests/fixtures/snapshots/` e
  `Cargo.toml.bak.*` (se algum for reintroduzido por tools) são
  agora tolerados.

### Changed
- Majors de dependências: `thiserror` 1.0 → 2.0, `scraper`
  0.20 → 0.27, `rand` 0.8 → 0.10
  (`OsRng`/`RngCore` migrados para `SysRng`/`TryRng` com
  propagação explícita de erro), `reqwest` 0.12 → 0.13
  (feature `rustls-tls` renomeada para `rustls`; `form` agora é
  uma feature opt-in explícita).
- Todas as mensagens `#[error("...")]` em `error.rs` traduzidas
  para inglês.
- Mensagens de erro e info em `io.rs` e `parse/video_id.rs`
  traduzidas para inglês.
- Strings `help` e `about` da CLI traduzidas para inglês;
  asserções de teste atualizadas para casar.
- Mensagens `about` e redaction-notice do binário `snapshot`
  traduzidas para inglês.
- `description` do `Cargo.toml` traduzida para inglês;
  `rust-version` fixado em `1.96.0`; dependência morta `anyhow`
  removida.
- `ci.yml` estendido com jobs `deny`, `audit`, `public-api`
  (com gate sigilo), `semver-checks`, `cargo-install`,
  `matrix-os`, `nightly`, `docs-link-check`, doctest, e
  doc-build.
- README reescrito em inglês com badges, tabela de flags,
  tabela de exit codes, instruções de instalação, baseline de
  performance e uma seção Documentation.
- llms.txt e llms-full.txt alinhados com os exit codes BSD, as
  17 flags cabeadas, os 6 alvos de cross-compile, e a superfície
  pública de API atual (módulos, conjunto `pub use`).
- `CONTRIBUTING.md` reescrito com um bloco bash limpo, as
  referências a `atomwrite` e `agent-teams-workflow`, e os 8
  gates de qualidade.

### Removed
- 26 arquivos `.bak` obsoletos de `src/`.
- Allowance órfã `BSD-2-Clause` do `deny.toml` (nenhuma
  dependência a usa após os upgrades abaixo).

## [0.1.0] - 2026-06-01

### Added
- Release público inicial.
- CLI Rust de binário único para baixar legendas do YouTube.
- Suporte a quatro formas de URL do YouTube: `watch`, `shorts`,
  `embed`, `youtu.be`.
- Pipeline de extração com dois provedores e fallback automático.
- Saída JSON estruturada via `--json`.
- Modo batch lendo URLs do stdin via `--batch`.
- Modo verbose emitindo eventos `tracing` no stderr.
- Cache local em arquivo com TTL de 24 horas em
  `~/.cache/youtube-legend-cli/`.
- Retry com backoff exponencial (1 s, 2 s, 4 s) para falhas
  transitórias.
- Circuit breaker em memória por provedor.
- Criptografia de token AES-256-CBC + PBKDF2-HMAC-SHA1
  (100 iterações).
- Limite de segurança em memória de 50 MiB no tamanho da
  legenda decodificada.
- CI no GitHub Actions: checagem de formatação, clippy, build,
  test, cross-compile, publish dry-run, MSRV.
- README bilíngue `pt-BR` / `en-US`.

## [0.0.1] - 2026-05-15

### Added
- PRD inicial com 13 seções obrigatórias.
- Notas de engenharia reversa a partir de tráfego ao vivo.
- 22 documentos `rules_rust` sob `docs_rules/`.
- Constituição com 15 princípios `PRINC-001` a `PRINC-015`.
- Especificação técnica cobrindo 14 módulos.
- Plano de implementação em 8 fases.
- 4 URLs de corpus em `tests/fixtures/corpus.txt`.

[0.2.8]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.1.0...v0.2.6
[0.1.0]: https://github.com/daniloaguiarbr/youtube-legend-cli/releases/tag/v0.1.0
[0.0.1]: https://github.com/daniloaguiarbr/youtube-legend-cli/releases/tag/v0.0.1

## [0.2.9] - 2026-06-14

### Added
- `docs/ARCHITECTURE.md` (diagrama mermaid do pipeline, mapa de
  módulos, contrato de stream, pipeline de provedores,
  cancelamento, seção de MSRV) e
  `docs/decisions/0010-deferred-doc-cfg-migration.md`
  (ADR em formato MADR explicando por que a migração
  `doc_auto_cfg → doc_cfg` está adiada para v0.3.0).
- Tabelas centralizadas `[lints.clippy]`, `[lints.rust]` e
  `[lints.rustdoc]` em `Cargo.toml` cobrindo 12 lints
  oficiais de rustdoc mais `clippy::doc_markdown`,
  `clippy::missing_errors_doc`, `clippy::missing_panics_doc` e
  `clippy::missing_safety_doc`. O bloco duplicado
  `#![warn/deny(...)]` em `src/lib.rs` foi removido em favor
  da fonte única de verdade.
- Superfície de `#[doc(alias = "...")]` expandida em `Cli`,
  `AppError` e a trait `Provider` para cobrir as queries de
  SEO que teriam sido servidas pelo atributo ainda instável
  `#[doc(keyword = "...")]`.

### Fixed
- 18 erros de clippy pegos pelos lints centralizados novos:
  `clippy::doc_markdown` (15 backticks ausentes em doc
  comments de módulo e struct) e `clippy::missing_errors_doc`
  (3 funções retornando `Result` sem seção `# Errors`) foram
  corrigidos em `src/lib.rs`, `src/cli.rs`, `src/commands/mod.rs`,
  `src/error.rs`, `src/parse/video_id.rs`, `src/provider/mod.rs`,
  `src/provider/provider_a.rs`, `src/provider/provider_b.rs`,
  `src/retry.rs`, `src/bin/snapshot.rs` e
  `src/secret_endpoints.rs`.
- `llms.txt` e `llms-full.txt` agora apontam para
  `github.com/daniloaguiarbr/youtube-legend-cli` (o handle
  público do GitHub) em vez do obsoleto `github.com/danilo/`.
  A seção `## Docs` em `llms.txt` é renomeada para
  `## Documentation` para casar com a spec llmstxt.org, e uma
  nova seção `## Architecture` resume o pipeline para
  consumidores LLM. O slug de categoria
  `web-programming::scraping` (depreciado pelo crates.io desde
  v0.2.7) é removido da linha `Categories` em `llms-full.txt`.
- Três snapshots obsoletos `ci.yml.bak.*` em `.github/workflows/`
  foram removidos (já estavam cobertos pelos patterns `*.bak.*`
  em `.gitignore`).

### Changed
- `cargo clippy --all-features -- -D warnings`,
  `cargo doc --no-deps --all-features`, e
  `RUSTDOCFLAGS="-D warnings" cargo doc --all-features` agora
  todos saem limpos. Este é o novo patamar de qualidade
  aplicado pelo CI.
