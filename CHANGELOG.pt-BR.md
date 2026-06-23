# Changelog

[English](CHANGELOG.md) | [Português Brasileiro](CHANGELOG.pt-BR.md)

## [0.3.3] - 2026-06-23

### Corrigido
- **GAP-AUD-2026-060**: envelope JSON de erro agora emitido no stdout para erros de validação pré-fetch quando `--json` está ativo; anteriormente stdout ficava vazio
- **GAP-AUD-2026-061**: campo `language_detected: false` adicionado ao envelope JSON de sucesso sinalizando que o idioma reflete o locale solicitado, não um detectado
- **GAP-AUD-2026-062**: marcadores de troca de falante `>>` de transcripts de entrevista agora removidos pelo parser `noteey_to_text`
- **GAP-AUD-2026-063**: documentação em `docs/AGENTS.md` e `docs/AGENTS.pt-BR.md` atualizada para usar `.content` (era `.body`) conforme o envelope JSON real
- **GAP-AUD-2026-064**: linha duplicada no stderr para erros de URL inválida eliminada (efeito colateral do fix GAP-060)
- **GAP-AUD-2026-065**: `byte_size` no envelope JSON agora reflete o tamanho do content limpo NFC, não o tamanho bruto do body HTML
- **GAP-AUD-2026-066**: flag `--verbose` agora funciona; anteriormente era uma flag morta sem efeito na saída de log
- **GAP-AUD-2026-067**: ruído de kill signal do stderr durante cleanup do Chromium eliminado via padrão `std::mem::forget(browser)`
- **GAP-AUD-2026-068**: limitação de `--format srt` com `provider-noteey` agora documentada no texto de `--help`
- **GAP-AUD-2026-069**: saída batch `--json` agora emite NDJSON válido (terminado com newline); anteriormente concatenava envelopes como `}{` quebrando parsers como `jq`

### Alterado
- Auditoria de documentação: `llms.txt`, `llms-full.txt`, `COOKBOOK.md`, `INTEGRATIONS.md` e seus equivalentes PT-BR atualizados para refletir a consolidação de providers da v0.3.2 (removidas referências stale a `ProviderA`, `ProviderB`, `ProviderHeadless`, `youtube-direct`, `--asr`, `--no-fallback`)

## [0.3.2] - 2026-06-21

### Quebra
- REMOVIDOS provedores: `youtube-direct`, `provider-a`, `provider-b`, `provider-headless`. Apenas `provider-noteey` permanece
- REMOVIDAS flags: `--asr`, `--no-fallback`, `--headless`. Estas flags agora produzem exit 2 (rejeição do clap)
- `--provider` agora aceita apenas `auto` e `provider-noteey`
- `provider-noteey` usa Chromium headless via `chromiumoxide 0.9.1` exclusivamente

### Adicionado
- `provider-noteey` como provedor exclusivo via Chromium headless (`chromiumoxide 0.9.1`)
- `BrowserFetcher` auto-baixa Chromium r1585606 pinado (versão 147.0.7693.0) em `~/.cache/youtube-legend-cli/browser/`
- `.new_headless_mode()` para compatibilidade com Chromium 147+
- `prepare_user_data_dir()` em `stealth.rs` limpa `Singleton{Lock,Cookie,Socket}` órfãos antes do launch
- Parser `noteey_to_text` em `src/parse/mod.rs` remove timestamps `MM:SS`/`HH:MM:SS` de transcripts noteey
- Função JS `findTranscriptRegion` isola o painel de transcript do header/nav da página
- 11 testes de regressão para `noteey_to_text` cobrindo timestamps, marcadores, Unicode NFC, cap 50 MiB

### Mudado
- Arquitetura simplificada para provedor único `provider-noteey`
- Chain `auto` agora contém apenas `provider-noteey`
- `ensure_chrome()` registra caminho do executável via `tracing::info`

### Corrigido (histórico — aplicado durante era multi-provedor antes da remoção)
- **GAP-AUD-003**: Eventos warn do `chromiumoxide` (loop `WS Invalid message: data did not match any variant of untagged enum Message`) silenciados via `EnvFilter::add_directive("chromiumoxide=error")` em `src/logging.rs`. A lógica do handler já descarta a mensagem CDP desconhecida mas o log disparava incondicionalmente. Operador pode sobrescrever com `YT_LOG_LEVEL=chromiumoxide=warn` para investigação detalhada
- **GAP-E2E-001**: Duplicação de log em 4 camadas por extração falhada colapsada para 1. Chamadas `info! "fetch_subtitle_started|completed"` em `provider_headless.rs:244,279`, `provider_a.rs:162,178`, `provider_b.rs:318,332`, `provider_youtube_direct.rs:153,160` rebaixadas para `debug!`. O warn do chain em `provider/mod.rs:263` e o erro canônico em `extract.rs:84` permanecem — eles são o sinal; os eventos em nível de provider eram ruído
- **GAP-E2E-009**: `--dry-run` agora retorna `ExitCode::SUCCESS` (0) e emite envelope JSON estável `dry_run_cache_miss` no stdout em vez de construir `AppError::NoSubtitle(NotPublished)` e sair com exit 66 (EX_NOINPUT). Scripts CI podem ramificar no campo `event`. Novo helper `output_dry_run` em `src/commands/mod.rs`
- **GAP-E2E-013**: Prefixo duplicado `config error:` removido. `main.rs:27` agora chama `eprintln!("{e}")` e deixa o Display de `AppError::Config` (que já inclui o prefixo) cuidar da mensagem. Operador vê `config error: <path>` exatamente uma vez
- **GAP-E2E-014**: Logs warn de retry em `retry.rs:52,63` rebaixados para `debug!`. Três tentativas × duas linhas warn não poluem mais o stderr sob `--log-level info`
- **GAP-E2E-015**: `Cli::validate()` agora retorna `AppResult<()>` em vez de `Result<(), String>`. A ponte `String → AppError::InvalidUsage` em `commands/mod.rs:42` foi removida. 12 testes existentes atualizados para afirmar `matches!(err, AppError::InvalidUsage(_))`
- **GAP-E2E-016**: `apply_config_overrides` baseado em sentinels (que comparava campos parseados contra defaults literais tipo `if self.timeout == 30`) substituído por bitmask `CliOverrideFlags` populado via `ArgMatches::value_source` no novo entry point `parse_with_overrides()`. A lógica anterior silenciosamente aplicava overrides de config incorretamente quando o operador digitava uma flag explicitamente com o mesmo valor do default; o novo bitmask reporta deterministicamente "foi setado na linha de comando?". 3 testes existentes atualizados e novo teste de regressão adicionado (`apply_config_overrides_explicit_default_does_not_get_overridden`)
- **GAP-E2E-017**: `parse_video_id_from_url` (em `commands/mod.rs`) roteado via `tracing::info!` em vez de `io::write_to_stderr` direto. A flag `--quiet` agora silencia de fato a linha verbose porque o EnvFilter do `tracing-subscriber` em `logging.rs` intercepta
- **GAP-E2E-018**: Logs warn em `player_js_cache.rs:136,158` rebaixados para `debug!`. Condições de race em cache miss não poluem mais o stderr sob `--log-level info`
- **GAP-E2E-024**: `extract_video_object_url` em `src/provider/provider_a.rs` refatorado para usar BFS com limite de profundidade e pré-alocação `Vec::with_capacity(8)`. A versão anterior empurrava cada valor JSON aninhado individualmente na fila e não tinha defesa contra referências cíclicas. Novos helpers `walk_video_objects` (recursivo com `MAX_DEPTH = 32`) e `video_object_caption` (extrator de objeto único) substituem a função monolítica. 3 novos testes de regressão cobrem resolução rasa, truncamento por aninhamento profundo e documentos largos de 1000 nós em orçamento de 100ms
- **GAP-E2E-025**: `provider_b.rs:251-253` agora retorna `AppError::NoSubtitle(NoSubtitleReason::NotPublished)` (exit 66) quando `sid`/`hash`/`hl` estão vazios, em vez de `AppError::ProviderUnavailable` (exit 69). Tokens de sessão vazios significam que o upstream não os gerou para este vídeo — semanticamente idêntico a "sem legendas publicadas". O comportamento anterior prendia scripts CI em loops infinitos de retry porque exit 69 sugere falha transitória
- **GAP-E2E-026**: HTTP 400 dos provedores upstream agora mapeia para `NoSubtitle(NotPublished)` (exit 66) em todos os provedores via `NoSubtitleReason::from_status(400)`. O mapeamento anterior era inconsistente: `provider_a.rs:118-122` já retornava `NoSubtitle` para 400, mas os outros 3 sites retornavam `ProviderUnavailable` (exit 69). A unificação trata 400 como "sem legendas" seguindo a convenção do endpoint YouTube `timedtext`. **BREAKING** para chamadores que ramificavam em exit 69 para respostas 400 — agora devem usar `NoSubtitleReason::from_status(400) == Some(NotPublished)`. 2 testes de integração atualizados
- **GAP-E2E-027**: `src/provider/robots.rs:73` não engole mais silenciosamente respostas `Ok(non-success)`. 5xx (transient) emite `tracing::warn!`, 4xx (definitivo) emite `tracing::debug!`. Operadores agora distinguem "robots.txt retornou 503 (problema upstream, comportamento pode mudar)" de "robots.txt retornou 404 (não existe, comportamento é definitivo)". A semântica fail-open é preservada em ambos os casos. 2 testes de contrato fixam a política de nível de log
- **GAP-E2E-028**: `ProviderYouTubeDirect::fetch_subtitle` agora consulta `robots.txt` antes de qualquer request via `super::robots::check_allowed(YOUTUBE_HOST, "/watch", USER_AGENT_IDENTITY).await?;` casando com o comportamento de `ProviderA` (linha 161) e `ProviderB` (linha 317). Conformidade com NFR-007 agora é uniforme nos 3 provedores. Nova constante `pub(crate) const YOUTUBE_HOST: &str = "www.youtube.com"` em `src/secret_endpoints.rs`. Nova suíte de teste de integração `tests/integration/provider_youtube_direct_wiremock.rs` (5 testes) cobre a lógica de match do robots-txt via wiremock
- **GAP-E2E-029**: Evento debug em `provider_youtube_direct.rs:153` agora usa `target: "events"` (consistente com os outros 6 callsites debug em providers A/B/headless) em vez do órfão `target: "youtube_decipher"`. Operadores com filtro de dashboard em `target = "events"` agora capturam o sinal de detecção de n-parameter consistentemente
- **GAP-E2E-030**: `provider_b.rs:140-148` agora retorna a nova variante `AppError::CaptchaChallenge { provider, kind }` quando o body contém `cf-turnstile` ou `h-captcha`, em vez de `AppError::ProviderUnavailable`. A nova variante preserva exit 69 (retrocompatível com scripts existentes) mas permite distinção programática via helper `AppError::is_captcha()`. Display inclui nome do provedor e implementação do captcha. 3 testes cobrem a nova variante e o helper
- **GAP-E2E-031**: `provider_youtube_direct.rs:321-329` agora retorna `AppError::TimedtextUpstreamError` (exit 70) para `Content-Type` inesperado em vez de `AppError::InvalidInput` (exit 64). O content-type vem do upstream YouTube, não do input do operador. A classificação anterior fazia o operador pensar que a CLI estava mal usada quando a causa real era upstream
- **GAP-E2E-032**: 6 sites em `src/parse/srv3.rs` (linhas 78, 93, 96, 111, 137, 147, 196) agora retornam `AppError::TimedtextUpstreamError` (exit 70) para falhas de parse do payload YouTube em vez de `AppError::InvalidInput` (exit 64). O body é originado do upstream, não do input do operador. A classificação anterior confundia erros de parse do YouTube Srv3/JSON3 com erros do operador CLI. 6 novos testes cobrem a reclassificação para `srv3_to_srt` (body vazio, sem cues `<text>`, `start` inválido, `dur` inválido) e `json3_to_srt` (body vazio, sem array `events[]`, sem eventos usáveis). O teste existente `rejects_empty_body` foi atualizado para casar com a nova variante
- **GAP-AUD-2026-033** (auditoria e2e de 2026-06-19): A feature `headless` do Cargo agora é habilitada por padrão (`Cargo.toml:79 default = ["headless"]`). O default anterior `[]` significava que o caminho `provider-headless` era inalcançável em builds default, mesmo sendo o único caminho viável contra o anti-bot do YouTube para IPs de datacenter e contra os endpoints CORS-restritos do downsub.com. O comportamento anterior prendia operadores que instalavam via `cargo install youtube-legend-cli` e nunca descobriam que precisavam de `--features headless`. A correção preserva a escapatória via `--no-default-features` para ambientes sem runtime Chromium/Chrome
- **GAP-AUD-2026-034** (auditoria e2e de 2026-06-19): `provider_headless.rs:24` agora importa `PROVIDER_A_PRIMARY_PAGE` (downsub.com) em vez de `PROVIDER_B_PRIMARY_PAGE` (downloadyoutubesubtitles.com). downsub.com é o site que operadores realmente usam no browser, tem SPA Vue.js com estrutura DOM previsível, e aceita a URL via o query param `?url=` (quando combinado com a interação de input-and-submit adicionada pelo GAP-AUD-2026-036)
- **GAP-AUD-2026-035** (auditoria e2e de 2026-06-19): `DOWNLOAD_JS` em `src/provider/provider_headless.rs` reescrito para casar com o DOM do downsub.com. O seletor anterior `document.querySelectorAll("a")` filtrado por `e.dataset.href && /get2\.php/.test(e.dataset.href)` é o contrato legado do downloadyoutubesubtitles.com; downsub.com renderiza `<button data-title="[TXT] Portuguese (auto-generated)">` envolvido em um anchor `<a href="...">`. O novo seletor `document.querySelectorAll("button, a")` filtra por `e.dataset.title` contendo tag de formato entre colchetes, depois sobe para o anchor pai para extrair a URL real de download. Budget de polling elevado para 45s × 1s (casa com o tempo de extração de até 30s reportado pelo usuário)
- **GAP-AUD-2026-036** (auditoria e2e de 2026-06-19): `drive_page` em `src/provider/provider_headless.rs` agora faz input-e-submit em vez de `goto` puro para uma URL `?url=`. A SPA Vue.js do downsub.com NÃO auto-processa o query param `?url=` — apenas popula o campo de input. O usuário (e agora a CLI) precisa setar o valor do input via setter nativo do `HTMLInputElement.prototype`, disparar evento `input` para que o Vue.js detecte a mudança, e clicar no botão submit. Nova constante `SUBMIT_JS` segura a sequência setter+dispatch+click. Fluxo de drive_page agora: abrir página `about:blank` → `goto(PROVIDER_A_PRIMARY_PAGE)` → sleep 5s para SPA hidratar → `page.evaluate(SUBMIT_JS)` → sleep 5s para o click handler disparar a extração
- **GAP-AUD-2026-037** (auditoria e2e de 2026-06-19): `drive_page` agora itera até 20 vezes sobre `browser.pages()`, escolhendo a primeira página não-home cuja URL contém `downsub.com` e re-executando `DOWNLOAD_JS` contra ela. O caminho anterior de evaluate único sempre falhava com `CdpError::ChannelClosed "Error -32000: Inspected target navigated or closed"` porque o submit do downsub dispara uma navegação dura que fecha o handle `Page` CDP original. O loop de retry re-resolve o target live após cada fronteira de `wait_for_navigation`, tolerando o comportamento de navegação da SPA. Sleep de 3s entre tentativas dá tempo suficiente para a SPA popular os botões de download por idioma

### Adicionado (fallback noteey)
- **`provider-noteey`** adicionado como fallback automático quando downsub.com degrada (GAP-AUD-2026-038). Quando `provider-headless` retorna `ProviderUnavailable` (site inalcançável, botões ausentes após 45 polls, button-sem-href, ou fetch não-200), o chain agora tenta noteey.com via um segundo provider headless. Ambos providers compartilham o cache on-disk do `BrowserFetcher` para evitar baixar Chromium duas vezes
- **`noteey_to_text`** em `src/parse/mod.rs` remove prefixos de timestamp `MM:SS` / `HH:MM:SS` de transcripts estilo noteey, descarta linhas só-com-marcador como `[Music]` e `(Applause)`, replica o cap de segurança de 50 MiB de `srt_to_text`, e normaliza Unicode para NFC
- **Enum `SubtitleFormat`** com variantes `Srt | NoteeyTranscript` e campo `SubtitleInfo::format_hint` para dispatch. `Srt` é `#[default]` então providers e consumers existentes não são afetados
- **`convert_format`** em `src/commands/mod.rs` agora aceita um parâmetro `format_hint` e escolhe o parser certo (`srt_to_text` para SRT, `noteey_to_text` para transcripts noteey). Rejeita `--format srt` quando a única fonte disponível é noteey via `AppError::InvalidUsage` com mensagem clara "use --format txt" — noteey não tem framing SRT então não fabricamos timestamps
- **Variante `ProviderChoice::ProviderNoteey`** em `src/cli.rs` para operadores fixarem o caminho só-noteey via `--provider provider-noteey`. Config TOML aceita o mesmo valor `provider-noteey`. Teste de integração `provider-noteey-wiremock` espelha o padrão existente `provider-headless-wiremock`
- **`src/provider/stealth.rs`** módulo compartilhado anti-fingerprint (GAP-AUD-2026-041 + GAP-AUD-2026-044). Exporta `pub async fn apply_stealth(page: &Page)` que enfileira `STEALTH_INIT_JS` via CDP `Page.addScriptToEvaluateOnNewDocument`. O init script mascara `navigator.webdriver`, polui `navigator.plugins` com 3 entradas padrão do Chromium, sobrescreve `navigator.languages`, troca o vendor WebGL `SwiftShader` por `Intel Inc.`, e instala um mock mínimo de `chrome.runtime`. 5 testes inline fixam o conteúdo do script; o teste contra um `chromiumoxide::Page` real fica para verificação manual contra `https://fingerprintjs.github.io/fingerprintjs/` (ver doc comment em `stealth.rs`)

### Corrigido
- **GAP-AUD-2026-038** (auditoria e2e de 2026-06-19): `provider_headless.rs` agora retorna `ProviderUnavailable` (exit 69) quando downsub.com degrada — especificamente quando o JS reporta `no matching button` após 45 polls, quando um botão matched não tem `href`, quando o fetch do botão por idioma retorna não-200, ou quando um erro JS-level desconhecido é surfaced. `NoSubtitle` (exit 66) agora é reservado para o caso genuíno "botão encontrado, status 200, body vazio" onde o downsub confirmou que o vídeo não tem legendas. A regra do chain `saw_no_subtitle` winner (`src/provider/mod.rs:270-273`) agora corretamente cai para `provider-noteey` quando o downsub reporta degradação
- **GAP-AUD-2026-039** (auditoria e2e de 2026-06-19): enum `ProviderOutcome` (interno a `ProviderChain`) classifica cada resposta de provider como `Subtitle(info, bytes)` ou `ChainError { source, error, degraded }`. HTTP 5xx e HTTP 429 são marcados com `degraded = true` — o chain passa por eles sem registrar `last_err` e sem marcar `saw_no_subtitle`. A trait `Provider` permanece inalterada; a classificação acontece dentro do wrapper do chain em torno de `fetch_subtitle`/`fetch_content`. Efeito: `provider_a` e `provider_b` retornando 5xx/429 transitório não envenenam mais o chain com `NoSubtitle`, então o fallback para `provider-headless` e `provider-noteey` agora dispara corretamente de qualquer posição do chain. 3 novos testes de chain cobrem o contrato degraded-skip
- **GAP-AUD-2026-040** (preventivo): `noteey_to_text` aplica o mesmo cap de 50 MiB que `srt_to_text` para prevenir OOM em vídeos longos
- **GAP-AUD-2026-041** (auditoria e2e de 2026-06-19): `provider_noteey.rs::drive_page` agora chama `stealth::apply_stealth(&page)` imediatamente após `browser.new_page("about:blank")` e antes de `page.goto(...)`. Os patches via CDP `Page.addScriptToEvaluateOnNewDocument` mascaram os sinais do Chromium headless que o Cloudflare Windsor.io (`r.wdfl.co/rw.js`) coleta no primeiro load do documento. Sem este patch, noteey.com atribui risk score alto e bloqueia a criação de sessão em IPs de datacenter
- **GAP-AUD-2026-044** (auditoria e2e de 2026-06-19, PRIORITÁRIO): `src/provider/stealth.rs` é o novo módulo compartilhado anti-fingerprint. `apply_stealth(page)` é invocado de AMBOS `provider_headless::drive_page` (downsub.com) E `provider_noteey::drive_page` (noteey.com) ANTES do primeiro `page.goto`. O init script mascara 5 sinais de fingerprint: `navigator.webdriver`, `navigator.plugins`, `navigator.languages`, `WebGLRenderingContext.prototype.getParameter` (mascara vendor string `SwiftShader`), e `window.chrome.runtime`. Flag complementar do Chromium `--disable-blink-features=AutomationControlled` agora está no `BrowserConfig` de ambos providers. A constante `HEADLESS_NAV_TIMEOUT = 60s` foi movida de cada provider para `stealth.rs` para eliminar drift silencioso entre os dois. Esta é a causa raiz unificada do problema de fingerprint Cloudflare observado em ambos providers headless — fechar este gap destrava o fallback downsub→noteey em ambiente datacenter

### Corrigido (v0.3.2)
- **GAP-AUD-2026-045**: erros CDP terminais em `provider_noteey.rs` submit/extract agora retornam `ProviderUnavailable` em vez de `Internal`, habilitando fallback automático do chain quando o alvo CDP fecha durante challenges Cloudflare
- **GAP-AUD-2026-046**: `noteey_extract_diagnostic` agora despeja os primeiros 500 chars do body da página via `tracing::warn!` quando o polling esgota sem encontrar transcripts, permitindo ao operador distinguir captcha interstitial, página vazia e render parcial sem re-executar manualmente
- **GAP-AUD-2026-054**: chain não mascara mais `BrowserNotFound` atrás de `NoSubtitle(NotPublished)`. O helper `remember_failure` agora protege sinais `BrowserNotFound` e `CaptchaChallenge` para que operadores recebam exit 69 com mensagem acionável "chromium/chrome not found" em vez de exit 66
- **GAP-AUD-2026-055**: `prepare_user_data_dir()` em `stealth.rs` limpa arquivos `Singleton{Lock,Cookie,Socket}` órfãos antes do `Browser::launch`, auto-curando o abort causado por locks órfãos de crashes anteriores. Profile do browser ancorado em `~/.cache/youtube-legend-cli/chrome-profile/` em vez do global `/tmp/chromiumoxide-runner/`

### Mudado (superfície noteey, histórico — antes da remoção de provedores)
- `provider_headless::ensure_chrome` agora é `pub` para que `provider-noteey` possa compartilhar o mesmo pin de Chromium e diretório de cache on-disk do `BrowserFetcher`

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

## [0.3.1] - 2026-06-19

### Corrigido
- **GAP-AUD-001**: Erros de parse e IO do arquivo de configuração agora retornam exit code 78 (`EX_CONFIG`) em vez de 64 (`EX_USAGE`), alinhando com `rules-rust-cli-stdin-stdout-config-observabilidade`. A nova variante `AppError::Config(String)` carrega o path do arquivo na mensagem de `Display`
- **GAP-AUD-002**: Documentado o exit code 2 do `clap::Error::exit()` para argumentos CLI inválidos (ex.: `--lang xx`). O código 2 é o comportamento canônico do clap conforme `rules-rust-cli-stdin-stdout-clap-exitcodes-erros` e é intencionalmente distinto de 64 (`AppError::InvalidUsage` para falhas de validação pós-parse)
- **GAP-AUD-003**: `ProviderHeadless` agora pina `BrowserFetcher` na revisão 1378488 do Chromium (compatível com o enum `Message` do `chromiumoxide` 0.9.1) via `with_version(BrowserVersion::Revision(Revision::new(1378488)))`, eliminando o loop de warning `WS Invalid message: data did not match any variant of untagged enum Message` durante a navegação

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

[0.3.3]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.9...v0.3.0
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
