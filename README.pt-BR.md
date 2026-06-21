# youtube-legend-cli

[English](README.md) | [Português Brasileiro](README.pt-BR.md)

[![docs.rs](https://docs.rs/youtube-legend-cli/badge.svg)](https://docs.rs/youtube-legend-cli)
[![Crates.io](https://img.shields.io/crates/v/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![v0.3.2](https://img.shields.io/badge/release-v0.3.2-blue.svg)](CHANGELOG.md)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/youtube-legend-cli.svg)](LICENSE)
[![MSRV 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-blue.svg)](rust-toolchain.toml)
[![CI](https://github.com/daniloaguiarbr/youtube-legend-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/daniloaguiarbr/youtube-legend-cli/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/crates/d/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

> CLI Rust não interativa que baixa legendas do YouTube via
> navegador headless Chromium (`chromiumoxide 0.9.1`), com interface
> Unix nativa `stdin`/`stdout`. Binário único, sem daemon, sem
> telemetria.

## Visão geral

`youtube-legend-cli` é um binário Rust estático único que transforma
qualquer URL do YouTube em um arquivo de legendas limpo. Não é
interativo, não tem daemon e nunca telefona para casa. A interface é
pura Unix: uma URL no `stdin` (ou como argumento posicional), o corpo
da legenda no `stdout` e todos os logs e progresso no `stderr`.

## Funcionalidades

- Extração via Chromium headless com `chromiumoxide 0.9.1` e auto-download via `BrowserFetcher`.
- Patches anti-fingerprint stealth (`navigator.webdriver`, plugins, languages, vendor WebGL).
- Cache local em arquivo indexado por `(video_id, language, format)` com TTL configurável (padrão 24h).
- Modo batch lendo uma URL por linha do `stdin`.
- Envelope JSON estruturado em `stdout` via `--json`.
- Normalização Unicode NFC e conversão transcript-para-texto.
- Limite de segurança em memória de 50 MiB no tamanho da legenda decodificada.
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
- `snapshot` — sonda o provedor e grava snapshots HTML
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
| `69`   | Provedor indisponível, browser não encontrado ou rate limited (`EX_UNAVAILABLE`) |
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


## Provedor (v0.3.2)

Desde v0.3.2 a CLI envia um único provedor de legendas:
`provider-noteey`. Ele dirige uma instância de Chromium headless
via `chromiumoxide 0.9.1` para extrair transcripts do noteey.com.

Se nenhum Chrome/Chromium local for encontrado, o `BrowserFetcher`
auto-baixa Chromium r1585606 (versão 147.0.7693.0) em
`~/.cache/youtube-legend-cli/browser/`. Use `$CHROME` para
sobrescrever o caminho do executável.

Patches anti-fingerprint em `src/provider/stealth.rs` mascaram
`navigator.webdriver`, populam `navigator.plugins`, sobrescrevem
`navigator.languages`, trocam o vendor WebGL de `SwiftShader`
para `Intel Inc.`, e mockam `window.chrome.runtime`.

### Seleção de provedor

| Valor | Efeito |
|-------|--------|
| `auto` | Usa `provider-noteey` (padrão) |
| `provider-noteey` | Seleção explícita do provedor noteey |

```bash
# Padrão — auto seleciona provider-noteey
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ"

# Seleção explícita de provedor
youtube-legend-cli --provider provider-noteey "https://youtu.be/dQw4w9WgXcQ"
```


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
