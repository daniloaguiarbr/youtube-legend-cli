# youtube-legend-cli

[English](README.md) | [Português Brasileiro](README.pt-BR.md)

[![docs.rs](https://docs.rs/youtube-legend-cli/badge.svg)](https://docs.rs/youtube-legend-cli)
[![Crates.io](https://img.shields.io/crates/v/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![v0.2.8](https://img.shields.io/badge/release-v0.2.8-blue.svg)](CHANGELOG.md)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/youtube-legend-cli.svg)](LICENSE)
[![MSRV 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-blue.svg)](rust-toolchain.toml)
[![CI](https://github.com/daniloaguiarbr/youtube-legend-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/daniloaguiarbr/youtube-legend-cli/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/crates/d/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

> CLI Rust não interativa que baixa legendas do YouTube via
> provedores terceirizados, com interface Unix nativa
> `stdin`/`stdout`. Binário estático único, sem daemon, sem telemetria.

## Visão geral

`youtube-legend-cli` é um binário Rust estático único que transforma
qualquer URL do YouTube em um arquivo de legendas limpo. Não é
interativo, não tem daemon e nunca telefona para casa. A interface é
pura Unix: uma URL no `stdin` (ou como argumento posicional), o corpo
da legenda no `stdout` e todos os logs e progresso no `stderr`.

## Funcionalidades

- Pipeline de extração com dois provedores (`provider_a`, `provider_b`)
  com fallback automático.
- Cache local em arquivo indexado por `(video_id, language, format)`
  com TTL configurável (padrão 24h).
- Modo batch lendo uma URL por linha do `stdin`.
- Envelope JSON estruturado em `stdout` via `--json`.
- Backoff exponencial (1s, 2s, 4s) com circuit breaker por provedor.
- Normalização Unicode NFC e conversão SRT-para-texto.
- AES-256-CBC mais assinatura de token PBKDF2 para o caminho de
  compatibilidade `provider_b`.
- Limite de segurança em memória de 50 MiB no tamanho da legenda
  decodificada.
- Tratamento gracioso de `SIGINT` e `SIGTERM`, sai com código 130.
- Zero telemetria: sem analytics, sem chamada de rede para casa.

## Início rápido

```bash
# Instalar do crates.io
cargo install youtube-legend-cli

# Ou compilar do código-fonte
cargo build --release

# Baixar legendas de um vídeo
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > legenda.txt

# Saída JSON estruturada
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY"

# Modo batch a partir do stdin
cat urls.txt | youtube-legend-cli --batch > legendas.txt

# Idioma específico
youtube-legend-cli --lang pt "https://youtu.be/NvZ4VZ5hooY"
```

## Exemplos

```bash
# Uma URL, saída em texto puro
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > legenda.txt

# Preservar timestamps SRT
youtube-legend-cli --format srt "https://youtu.be/dQw4w9WgXcQ" > legenda.srt

# Português brasileiro
youtube-legend-cli --lang pt "https://youtu.be/dQw4w9WgXcQ"

# Batch a partir de um arquivo
youtube-legend-cli --batch < urls.txt > legendas.txt

# Envelope JSON no stdout, logs no stderr
youtube-legend-cli --json --verbose "https://youtu.be/dQw4w9WgXcQ"
```

## Alvos

Binários pré-construídos são produzidos e testados para:

- `x86_64-unknown-linux-gnu` (glibc dinâmico)
- `x86_64-unknown-linux-musl` (totalmente estático)
- `aarch64-unknown-linux-musl` (ARM64 estático)
- `x86_64-pc-windows-msvc` (Windows 64-bit)
- `x86_64-apple-darwin` (cross-compile via `osxcross`, `continue-on-error: true` no CI)
- `aarch64-apple-darwin` (cross-compile via `osxcross`, `continue-on-error: true` no CI)

O alvo `aarch64-apple-darwin` está na matriz do CI mas é melhor
compilado em um host que disponibilize `osxcross`; a árvore de
código-fonte em si é portável para qualquer alvo Rust Tier-1.

## Binários companheiros

O crate envia dois binários:

- `youtube-legend-cli` — o buscador de legendas (padrão).
- `snapshot` — sonda ambos os provedores e grava snapshots HTML
  redacted sob `tests/fixtures/snapshots/<date>/` para detecção de
  drift. O arquivo `src/secret_endpoints.rs` (gitignored) é
  consumido via `#[path = "..."]` para que os hostnames upstream
  nunca entrem no rustdoc publicado. Execute com
  `cargo run --bin snapshot`.

## MSRV

A Versão Mínima de Rust Suportada é **1.88.0**, declarada em
`rust-toolchain.toml`. O job MSRV no CI compila e testa o crate
nessa versão em cada push.

## Contratos de stream

- `stdout` é reservado exclusivamente para o corpo da legenda (ou
  o envelope `--json`).
- `stderr` é reservado exclusivamente para logs, progresso e
  mensagens de erro humanas.
- `stdin` aceita uma única URL, um batch de uma URL por linha, ou
  entrada via flag `--batch`.

## Flags

| Flag             | Descrição                                  | Padrão     |
|------------------|--------------------------------------------|------------|
| `--lang`         | `en`, `pt`, `es`, `fr`, `de`, `it`, ou formas BCP 47 como `pt-BR` / `pt_BR.UTF-8` | `en`       |
| `--format`       | `txt` (puro) ou `srt` (preservado)         | `txt`      |
| `--timeout`      | Timeout HTTP em segundos                   | `30`       |
| `--verbose`      | Emite eventos de tracing em stderr         | `false`    |
| `--quiet`        | Suprime todo stderr que não seja erro      | `false`    |
| `--json`         | Emite envelope JSON no stdout             | `false`    |
| `--batch`        | Lê múltiplas URLs do stdin                 | `false`    |
| `--user-agent`   | Sobrescreve o User-Agent padrão           | nome do crate |
| `--cache-ttl`    | TTL do cache em horas                      | `24`       |
| `--no-cache`     | Pula leituras do cache                     | `false`    |
| `--config`       | Caminho para um arquivo de config TOML     | nenhum     |
| `--log-level`    | `error`, `warn`, `info`, `debug`, `trace`  | `warn`     |
| `--log-format`   | `text` ou `json`                           | `text`     |
| `--color`        | `auto`, `always`, `never`                  | `auto`     |
| `--no-progress`  | Suprime barras de progresso em stderr      | `false`    |
| `--dry-run`      | Pula I/O de rede; serve leituras só do cache | `false`  |
| `--yes`          | Assume sim para qualquer confirmação       | `false`    |

## Códigos de saída

A CLI segue a convenção BSD `sysexits.h` para que o tooling POSIX
downstream possa ramificar por categoria. Veja
[`src/error.rs`](src/error.rs) para o mapeamento canônico.

| Código | Significado                                            |
|--------|--------------------------------------------------------|
| `0`    | Sucesso                                                |
| `64`   | Uso ou entrada inválida (`EX_USAGE`)                  |
| `65`   | URL inválida (`EX_DATAERR`)                           |
| `66`   | Nenhuma legenda para o vídeo (`EX_NOINPUT`)           |
| `69`   | Todos os provedores indisponíveis, ou rate limited, ou `robots.txt` `Disallow` (`EX_UNAVAILABLE`) |
| `70`   | Erro interno / I/O / HTTP / timeout / crypto (`EX_SOFTWARE`) |
| `78`   | Erro de configuração no TOML de `--config` (`EX_CONFIG`) |
| `130`  | Recebido `SIGINT` / `SIGTERM` (primeiro sinal cooperativo, segundo força saída) |

Em HTTP 429 a CLI honra o header `Retry-After` tanto em
delta-seconds quanto no formato RFC 2822 HTTP-date (fallback 60s
quando ausente, limitado a 300s) antes de tentar novamente.

## Instalação

```bash
# Do crates.io
cargo install youtube-legend-cli

# Do checkout local
cargo install --path .

# Verificar
youtube-legend-cli --version
```

Requer Rust 1.88.0 ou mais recente. Veja o campo `rust-version` em
`Cargo.toml`.

## Baseline de performance


## Provedores (v0.3.0+)

O crate envia quatro provedores de legenda. A CLI tenta-os na
ordem documentada abaixo em `--provider` até que um retorne uma
legenda não vazia. A trait `Provider` permanece inalterada desde
v0.2.x — todo provedor implementa o mesmo contrato `fetch_subtitle`,
então trocar ou encadear é transparente para o restante do pipeline.

| Provedor | Fonte | Quando funciona | Papel de fallback |
|----------|-------|-----------------|-------------------|
| `youtube-direct` | Página watch do YouTube + `ytInitialPlayerResponse` + endpoint `timedtext` | Padrão para quase todo vídeo com qualquer trilha de legenda (manual ou ASR) | Primário na cadeia `auto` desde v0.3.0 |
| `provider_a` | Serviço HTTP terceirizado análogo a downsub.com | Vídeos que o serviço terceirizado indexou | Secundário na cadeia `auto` |
| `provider_b` | Segundo serviço HTTP terceirizado com assinatura AES-256-CBC + PBKDF2 | Vídeos que o segundo serviço indexou | Terciário na cadeia `auto` |
| `provider_headless` | Chromium local dirigido por `chromiumoxide` (gateado pela feature `headless`) | Endpoints protegidos por Cloudflare ou browser-gated que bloqueiam os provedores HTTP puros | Fallback opt-in; nunca habilitado por padrão |

### Provedor YouTube Direct

O provedor YouTube-direct é a resolução de `GAP-001`. Ele busca
a página watch, faz parse de `ytInitialPlayerResponse`, escolhe a
entrada certa de `captionTracks` e baixa o payload do timedtext
diretamente de `https://www.youtube.com/api/timedtext`. Trata
legendas manuais e auto-geradas, parâmetros de assinatura, o
desafio `n`, e cacheia a tabela de operações do `player.js` em
`~/.cache/youtube-legend-cli/player/` por sete dias.

```bash
# Manual ou auto-gerada em português, caindo pela cadeia
youtube-legend-cli https://youtu.be/Ze0i7zxpyrw --lang pt --provider youtube-direct --asr
```

A flag `--asr` faz o provedor preferir a trilha auto-gerada
(`kind: "asr"`) mesmo quando uma trilha manual também está
presente. Sem `--asr`, o provedor escolhe a trilha manual
quando disponível e só cai para ASR quando nada mais existir.
A flag `--no-fallback` restringe a cadeia ao provedor escolhido
para que um único upstream mal-comportado não mascare o
comportamento real dos outros.

### Seleção de provedor

`--provider` aceita os valores na tabela abaixo. `auto` é o
padrão e a escolha recomendada para pipelines de produção. A
ordem da cadeia em modo `auto` é `youtube-direct` depois
`provider_a` depois `provider_b`; `provider_headless` só entra
na cadeia quando o binário foi compilado com a feature `headless`.

| Valor | Efeito |
|-------|--------|
| `auto` | Tenta `youtube-direct` primeiro, depois `provider_a`, depois `provider_b`, depois o headless se habilitado. |
| `youtube-direct` | Apenas o provedor YouTube-direct. Desabilita todos os outros provedores para a execução. |
| `provider_a` | Apenas `provider_a`. Reproduz a cadeia padrão de v0.2.x. |
| `provider_b` | Apenas `provider_b`. Reproduz o caminho alternativo de v0.2.x. |
| `provider_headless` | Apenas o provedor headless. Requer `--features headless` em build time. |

As flags `--provider` e `--asr` se compõem: `--asr` se propaga
para o provedor escolhido, que decide se prefere a trilha
auto-gerada. `--asr --provider provider-a` é rejeitado com exit
code `64` (`EX_USAGE`) porque os provedores terceirizados não
expoem uma seleção manual-versus-ASR.


Três micro-benchmarks vivem em `benches/cache_bench.rs`:

- `cache_key_compose` — compõe o nome de arquivo do cache a partir de (video_id, lang, format)
- `url_length_check` — valida URL contra o limite de 2048 bytes
- `locale_parse_primary_subtag` — normaliza BCP 47 para ISO 639-1


Execute com `cargo bench --bench cache_bench`. Baseline no host
x86_64-unknown-linux-gnu do mantenedor (2026-06-14, profile release,
1000 amostras):
- `cache_key_compose`: ~34 ns/iter
- `url_length_check`: 0 ns/iter (sub-ns, arredondado para baixo)
- `locale_parse_primary_subtag`: ~6 ns/iter

## Documentação

- [docs.rs/youtube-legend-cli](https://docs.rs/youtube-legend-cli) —
  referência de API para cada item público.
- [CHANGELOG.md](CHANGELOG.md) — histórico de releases.
- [CONTRIBUTING.md](CONTRIBUTING.md) — fluxo de desenvolvimento.
- [SECURITY.md](SECURITY.md) — divulgação de vulnerabilidades.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — padrões da comunidade.
- [llms.txt](llms.txt) — ponto de entrada amigável para LLM.
- [llms-full.txt](llms-full.txt) — referência completa amigável para LLM.
- [docs/agent-teams-workflow.md](docs/agent-teams-workflow.md) —
  playbook do Agent Teams usado para entregar v0.2.6.
- [docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md](docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md) —
  ADR-0009 (serialização do Cargo.toml sob Agent Teams).
- `docs_prd/prd_youtube-legend-cli.md` — PRD completo (Constitution: PRINC-001 a PRINC-015 embutida em §13).
- `docs_prd/spec_tecnica.md` — contratos de módulos.
- `docs_prd/plano_implementacao.md` — fases de desenvolvimento.

## Segurança

Veja [`SECURITY.md`](SECURITY.md) para a tabela de versões
suportadas, o modelo de ameaça e o canal privado de divulgação de
vulnerabilidades.

## Código de Conduta

Este projeto segue o
[Contributor Covenant 2.1](CODE_OF_CONDUCT.md).

## Contribuindo

Veja [`CONTRIBUTING.md`](CONTRIBUTING.md) para o fluxo de
desenvolvimento, expectativas de MSRV, regras de estilo e a
política `no Co-authored-by`.

## Licença

Duplamente licenciado sob [MIT](LICENSE-MIT) ou
[Apache-2.0](LICENSE-APACHE), a seu critério.
