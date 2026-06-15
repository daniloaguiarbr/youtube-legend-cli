# gaps.md — youtube-legend-cli

Registro vivo de problemas conhecidos, suas causas raiz, consequências e
caminhos de solução. Cada entrada segue o mesmo formato canônico para
permitir triagem determinística e priorização incremental.

Convenções:
- IDs `GAP-NNN` estáveis e monotônicos.
- Status: `ABERTO` (sem trabalho em andamento), `EM-ANÁLISE` (investigando),
  `PLANEJADO` (solução aprovada, aguardando execução), `EM-EXECUÇÃO`,
  `RESOLVIDO` (verificável), `DESCARTADO` (decisão consciente de não resolver).
- Severidade: `CRÍTICO` (impede caso de uso principal), `ALTO` (degrada
  experiência principal), `MÉDIO` (afeta nicho), `BAIXO` (cosmético).
- Esforço: estimativa em pontos de complexidade relativa (1 = trivial,
  5 = substancial).
- Relações `causa → efeito` explícitas e ordenadas.


## GAP-001 — CLI não baixa legendas auto-geradas do YouTube sem depender de API key

- **Data de abertura**: 2026-06-14
- **Severidade**: ALTO
- **Esforço estimado**: 5
- **Status**: RESOLVIDO (parcial) em 2026-06-15 via commit local. ProviderYouTubeDirect implementado com M1-M5 e M3.5. 167 testes verdes. NÃO publicado em GitHub/crates.io — apenas commits locais pendentes de aprovação do usuário para tag v0.3.0 e push.
- **Reportado por**: teste manual com `https://youtu.be/Ze0i7zxpyrw` em
  2026-06-14. O usuário baixou a mesma legenda via `downsub.com/lang/pt`
  no mesmo dia (arquivo
  `[Portuguese (auto-generated)] SOCIALISMO da GERAAO-Z e o MESMO SOCIALISMO
  de SEMPRE [DownSub.com].txt`), comprovando que a legenda está
  publicamente acessível para aquele vídeo. Nossa CLI retornou
  `exit 66 EX_NOINPUT` com a mensagem
  `no captions published for this video` em ambos os provedores
  configurados (`provider_a` e `provider_b`).

### Problema

A CLI atual é incapaz de recuperar legendas auto-geradas (ASR) do
YouTube para uma classe significativa de vídeos. O caso concreto
reproduzido foi o vídeo `Ze0i7zxpyrw` com legenda
`Portuguese (auto-generated)`, publicamente servida pelo YouTube e
acessível via `downsub.com`, mas não recuperável por nenhum dos
provedores da CLI.

### Consequências do problema

- `C-1` — Usuários que rodam a CLI em pipelines automatizados (integração
  contínua, RSS feeds, processamento de mídias em lote) recebem
  `EX_NOINPUT` (exit 66) em vídeos que sabidamente têm legendas.
- `C-2` — A CLI é marcada como quebrada em casos de uso reais, forçando
  fallback para serviços web com UI (downsub.com, savesubs.com) que
  adicionam watermark ao nome do arquivo.
- `C-3` — Perda de cobertura em vídeos que dependem exclusivamente de
  ASR (canais de notícias, lives, conteúdo educacional sem transcrição
  manual) — a maioria do conteúdo do YouTube.
- `C-4` — A CLI promete, no `README.md`, download de legendas "through
  third-party providers", mas a taxa de acerto real é desconhecida e
  degradada. Falsa promessa de cobertura.
- `C-5` — Impossibilidade de auditar/medir a taxa de acerto porque
  não há provider de fallback que opere em frente diferente.

### Causa raiz do problema

A CLI não consulta o YouTube diretamente. Toda a obtenção de legendas
pasa por um dos três provedores em
`src/provider/`:

- `provider_a.rs` — consulta um serviço third-party HTTP análogo ao
  downsub.com.
- `provider_b.rs` — consulta um segundo serviço third-party HTTP, com
  fallback para `a.<lang>` (auto-generated) que **ainda depende do
  mesmo serviço third-party**.
- `provider_headless.rs` — alternativa via Chromium controlado via
  `chromiumoxide`, também apontando para o mesmo serviço third-party
  (`PROVIDER_B_PRIMARY_PAGE` em `src/secret_endpoints.rs`), com o
  JavaScript `DOWNLOAD_JS` que procura âncoras
  `a[data-href*="get2.php"]` na página de resultado do serviço.

A causa raiz é arquitetural: **não existe provedor que fale o protocolo
do YouTube diretamente**. Consequência direta:

- `R-1` — `provider_a` e `provider_b` dependem de índices e
  disponibilidade de serviços third-party. Quando o serviço não indexou
  o vídeo, a CLI falha mesmo quando o YouTube serve a legenda.
- `R-2` — `provider_headless` automatiza a UI do `provider_b`, então
  herda as mesmas limitações de índice.
- `R-3` — Nenhum dos três provedores implementa a técnica do
  `https://www.youtube.com/api/timedtext?v=<id>&lang=<lang>&kind=<kind>`
  nem o scraping do `ytInitialPlayerResponse.captions.
  playerCaptionsTracklistRenderer.captionTracks[].baseUrl`.
- `R-4` — O YouTube serve legendas (manuais e auto-geradas) através de
  um endpoint público (`/api/timedtext` e via `baseUrl` em
  `captionTracks`) que **não requer YouTube Data API key**, embora
  frequentemente exija o parâmetro `signature` decifrado pelo JavaScript
  do player para vídeos mais protegidos.

### Solução

Adicionar um provedor nativo YouTube (`ProviderYouTubeDirect` em
`src/provider/provider_youtube_direct.rs`) que implementa a estratégia
canônica usada por `yt-dlp`, `youtube-transcript-api` e pelo próprio
`downsub.com` (apenas o subconjunto que não exige UI de navegador):

1. `GET https://www.youtube.com/watch?v=<video_id>` com `User-Agent`
   realista e cabeçalhos `Accept-Language` coerentes com a língua
   solicitada. Resposta HTML contém o JSON
   `ytInitialPlayerResponse` embutido em tag `<script>`.
2. Parsear esse JSON com `serde_json::Value` e navegar até
   `captions.playerCaptionsTracklistRenderer.captionTracks`,
   que é um array com objetos `{baseUrl, languageCode, name,
   vssId, kind}`.
3. Para cada `captionTrack`, aplicar filtros:
   - `languageCode == requested` (case-insensitive) **ou** começa com
     o prefixo da língua.
   - Preferência: `kind == "asr"` quando o usuário pediu legenda
     auto-gerada via flag nova `--asr` ou `--auto-generated`.
4. Resolver o `baseUrl`:
   - Se o `baseUrl` contém `&sig=` ou `&signature=`, decifrar usando
     a tabela de operações extraída do JavaScript do player
     (`base.js` da versão atual do player). Esta é a parte
     significativamente complexa; estratégia mais barata é
     cachear a `player_url` e usar a versão do player detectada na
     própria página, reaproveitando o trabalho do projeto
     `youtube-transcript-api` (Python) ou
     `rusty_ytdl` (Rust) como referência.
   - Se o `baseUrl` não tem signature (caso comum para vídeos
     antigos ou de baixo tráfego), `fetch` direto.
5. Adicionar parâmetros `&fmt=json3` ou `&fmt=srv3` para obter SRT
   canônico do YouTube (mais limpo que o `srv1` legacy).
6. Converter o JSON3 ou Srv3 para SRT localmente, reutilizando a
   pipeline existente em `src/text.rs` e `src/parse/`.

### Benefícios da solução

- `B-1` — Cobertura expande para a maioria esmagadora dos vídeos
  do YouTube que têm qualquer forma de legenda (manual ou ASR), sem
  depender de serviços externos.
- `B-2` — Reduz dependência operacional: terceiros podem cair, mudar
  de URL ou bloquear scraping. YouTube é a fonte primária.
- `B-3` — Elimina watermark do downsub.com do nome do arquivo
  entregue ao usuário (o usuário recebe nome limpo, com sufixo
  controlado por `--output` ou pelo idioma selecionado).
- `B-4` — Habilita `--auto-generated` (ou `--asr`) como flag
  explícita, dando controle fino ao usuário sobre preferir ASR
  mesmo quando há legenda manual.
- `B-5` — Permite auditoria de cobertura: podemos medir taxa de
  acerto do provider direto vs. provedores third-party.
- `B-6` — Reduz superfície de manutenção: um endpoint oficial
  do YouTube é mais estável que N serviços third-party que mudam
  layout HTML a cada refator.

### Como solucionar

Implementação incremental, em 4 marcos verificáveis (cada marco
termina com `cargo test` verde e `--target x86_64-unknown-linux-gnu`
verificado):

- **Marco M1 — Player response parser** (esforço 2): novo módulo
  `src/provider/youtube/player_response.rs` que faz GET da watch
  page, extrai `ytInitialPlayerResponse` via regex/parser
  `serde_json::from_str`, valida o schema mínimo. Adicionar
  `ProviderYouTubeDirect::list_tracks(...)` retornando
  `Vec<CaptionTrack>`. Critério de aceitação: teste de
  snapshot contra uma watch page real, congelada em
  `tests/fixtures/youtube_watch_*.html`.

- **Marco M2 — Timedtext fetcher sem signature** (esforço 1):
  consumir `baseUrl` direto de tracks sem `&sig=`, baixar Srv3,
  converter para SRT. Critério: rodar CLI contra 5 vídeos
  públicos sem signature e validar que o SRT sai com timestamps
  e blocos `-->` preservados.

- **Marco M3 — Signature decipher** (esforço 2): portar a lógica
  de decipher de `base.js`. Estratégia: `PlayerJsCache` em
  `~/.cache/youtube-legend-cli/player/<version>.js`, atualizado
  quando o player muda. Reusar regex do
  `rusty_ytdl::sig::decipher` ou portar a versão TypeScript do
  `youtube-transcript-api`. Critério: rodar CLI contra
  `Ze0i7zxpyrw` (vídeo do bug report) e validar que o SRT sai
  não-vazio.

- **Marco M4 — Integração e flag** (esforço 1): adicionar
  `--provider youtube-direct` (padrão quando outros falham),
  `--asr` para preferir auto-gerada. Reordenar `mod.rs`
  para que o novo provider seja tentado primeiro. Critério:
  benchmark de taxa de acerto (provider direto vs. third-party)
  em 50 vídeos públicos; documentação no `README.md`.

Regras rust do graphrag aplicáveis durante a execução:

- `rules-rust-busca-estrutural-lint-e-rewrite-codigo` para refator
  do `mod.rs` e dos call sites.
- `rules-rust-testes` (fundamentos, avançados, property-based,
  async) — `#[tokio::test]` para todos os call sites async, testes
  de snapshot com `insta` para HTML congelado, property tests
  com `proptest` para o conversor Srv3→SRT.
- `rules-rust-seguranca-ffi-deserial-dos` — `serde_json::Value`
  com `arbitrary_limit` no parse do `ytInitialPlayerResponse` para
  evitar DoS por JSON gigante.
- `rules-rust-cicd-testes` — gate no CI exige os 3 targets
  (`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`,
  `aarch64-unknown-linux-musl`) verdes antes do merge.
- `rules-rust-cache-redis-config` — reusar padrão de cache
  para `PlayerJsCache` em XDG cache dir com TTL de 7 dias.
- `rules-rust-tratamento-erros-fundamentos` — classificar erros
  novos: `SignatureDecipherFailed`, `PlayerResponseMissing`,
  `CaptionTrackNotFound`, `TimedtextUpstreamError`.
- `rules-rust-multi-idioma-i18n-automatico` — `--lang pt-BR` deve
  ser normalizado para `pt` antes de matching com `languageCode`.
- `rules-rust-web-scraping-completo` — robots.txt do YouTube
  (`robots.rs` já existe) e rate limit por IP respeitados.
- `rules-rust-api-rest-tls-authn` — `reqwest` com rustls já
  configurado; manter `cookies` feature.

### Causa → Efeito (relações ordenadas)

- `R-1 → C-1` (dependência de índice third-party causa exit 66 em
  pipelines)
- `R-1 → C-2` (usuário migra para downsub.com e recebe watermark)
- `R-1 → C-3` (perda de cobertura em vídeos só com ASR)
- `R-2 → C-3` (headless herda limitação do provider B)
- `R-2 → C-5` (headless também falha quando provider B não tem
  índice; não há alternativa YouTube-direct)
- `R-3 → C-4` (README promete cobertura que a arquitetura não
  entrega)
- `R-4 → C-1` (técnica do timedtext existe e é pública; o fato de
  não estarmos usando é decisão de design reversível)
- `M1..M4 → R-3` (implementar o provedor direto elimina a causa
  raiz R-3)
- `M1..M4 → R-1` (parcialmente; o provider direto ainda depende
  de YouTube servir a legenda, mas o índice não é mais
  problema)
- `M1..M4 → C-1`, `C-2`, `C-3`, `C-4`, `C-5` (resolução de todas
  as consequências observáveis)
## Histórico de GAPs Resolvidos (auditoria 2026-06-15)

Esta seção cataloga GAPs que foram resolvidos em commits passados
mas não estavam documentados no registro canônico. Mantida
retroativamente para rastreabilidade.

### GAP-007 — Sigilo: `pub mod secret_endpoints` exposto
- Data de resolução: v0.2.x (anterior a 2026-06-15)
- Severidade: ALTO
- Status: RESOLVIDO
- Causa raiz: `pub mod secret_endpoints;` em `src/lib.rs` vazava
  hosts dos provedores third-party via rustdoc
- Solução: alterado para `pub(crate) mod secret_endpoints;` em
  `src/lib.rs:7`. Binário `snapshot` consome via
  `#[path = "../secret_endpoints.rs"]`
- Gate de proteção: `ci.yml` job `public-api` falha o build se a
  regressão voltar

### GAP-008 — Teste de integração offline-cache ausente
- Data de resolução: v0.2.x
- Severidade: MÉDIO
- Status: RESOLVIDO
- Solução: `tests/integration/offline_cache.rs` exercita round-trip
  `(read cache, no network, plain text output)` com fixture URL

### GAP-010 — Compliance com robots.txt
- Data de resolução: v0.2.x
- Severidade: ALTO (legal e ético)
- Status: RESOLVIDO
- Solução: `src/provider/robots.rs` consulta robots.txt antes de
  qualquer request; `Disallow` tratado como `EX_UNAVAILABLE` (exit 69)

### GAP-011 — Tracing::instrument ausente
- Data de resolução: v0.2.x
- Severidade: MÉDIO
- Status: RESOLVIDO
- Solução: `#[tracing::instrument]` em 14 entry points públicos
  internos (commands::run, extract::run, batch::run,
  ProviderA::fetch_subtitle, ProviderB::fetch_subtitle,
  ProviderChain::fetch_subtitle, cache::{read,write,path},
  retry::retry_with_backoff, parse::{extract_video_id,srt_to_text},
  io::read_url_from_stdin, commands::batch::dedup_set,
  provider::robots::check)

### GAP-012 — Testes de provedores com wiremock
- Data de resolução: v0.2.x
- Severidade: MÉDIO
- Status: RESOLVIDO
- Solução: `tests/integration/provider_a_wiremock.rs` e
  `provider_b_wiremock.rs` exercitam ambos provedores contra
  mocks wiremock; CLI testada sem tocar upstream ao vivo

### GAP-017 — Superfície `pub use` inflada
- Data de resolução: v0.2.x
- Severidade: BAIXO
- Status: RESOLVIDO
- Solução: `pub use` em `src/lib.rs` reduzido de 14+ símbolos
  para 2 re-exports justificados (`Cli`, `FormatArg`,
  `LanguageArg`, `AppError`, `AppResult`, `NoSubtitleReason`)

### GAP-018 e GAP-027 — Módulo text exposto desnecessariamente
- Data de resolução: v0.2.x
- Severidade: BAIXO
- Status: RESOLVIDO
- Solução: `pub mod text` alterado para `pub(crate) mod text` —
  normalização Unicode NFC é helper interno

### GAP-019 — Configuração clippy permissiva
- Data de resolução: v0.2.x
- Severidade: MÉDIO
- Status: RESOLVIDO
- Solução: `clippy.toml` com 3 métodos proibidos,
  `cognitive-complexity-threshold = 30`,
  `too-many-arguments-threshold = 8`

### GAP-022 e GAP-024 — Matriz cross-compile limitada
- Data de resolução: v0.2.x
- Severidade: ALTO (portabilidade)
- Status: RESOLVIDO
- Solução: 6 targets via job `cross-compile` do `ci.yml`:
  `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-musl`, `x86_64-pc-windows-msvc`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin` (últimos 2
  com `continue-on-error: true` por exigirem osxcross)

### GAP-023 — Heurística de fallback para provider-A HTML drift
- Data de resolução: v0.2.x
- Severidade: ALTO
- Status: RESOLVIDO
- Solução: `src/provider/provider_a.rs` walks
  `JSON-LD VideoObject` block quando seletor primário
  `scraper::Html` falha

### GAP-026 — Testes de read_url_from_stdin ausentes
- Data de resolução: v0.2.x
- Severidade: MÉDIO
- Status: RESOLVIDO
- Solução: `tests/integration/io.rs` cobre 3 shapes (single URL,
  batch via `--batch`, one-URL-per-line)


## Meta-Gaps Identificados (auditoria 2026-06-15)

### META-GAP-A — gaps.md incompleto como registro vivo
- Data de abertura: 2026-06-15
- Severidade: MÉDIO
- Status: RESOLVIDO nesta entrega
- Causa raiz: 13 GAPs (007 a 027) foram resolvidos em commits
  passados mas não foram registrados retroativamente
- Solução: seção "Histórico de GAPs Resolvidos" adicionada acima
- Prevenção: gate de auditoria no release process

### META-GAP-B — DoS protection ausente em player_response.rs
- Data de abertura: 2026-06-15
- Severidade: ALTO
- Status: ABERTO
- Causa raiz: `serde_json::from_str(&raw_json)` em
  `src/provider/youtube/player_response.rs:213` SEM
  `arbitrary_limit` explícito. Plano M1 do GAP-001 previa
  proteção DoS
- Solução proposta: usar
  `serde_json::Deserializer::from_str(&raw_json)
  .with_depth_limit(64)` ou limite de bytes no body antes do
  parse
- Esforço: 1 (trivial)
- Risco: ytInitialPlayerResponse pode ser arbitrariamente
  grande

### META-GAP-C — Codex OAuth 401 degradando GraphRAG
- Data de abertura: 2026-06-15
- Severidade: MÉDIO
- Status: ABERTO (bloqueio externo à CLI)
- Causa raiz: `codex 0.139.0` retorna 401 ao chamar
  `wss://api.openai.com/v1/responses`; OAuth token ausente
- Impacto: GraphRAG cai para FTS5 fallback; busca semântica
  limitada
- Workaround: executar `codex login` manualmente
