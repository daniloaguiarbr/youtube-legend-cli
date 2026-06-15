# Guia de Testes — youtube-legend-cli

> Uma suíte de testes categorizada que espelha o pipeline de providers.

## Por Que Testes Categorizados

`youtube-legend-cli` envia um binário, um crate de biblioteca,
três examples e um benchmark. Os testes estão divididos em quatro
categorias que casam com a superfície de build e runtime:

- TESTES UNITÁRIOS sob módulos `#[cfg(test)]` dentro de `src/`.
  Rápidos, determinísticos, sem rede. Exercitados por
  `cargo test --lib`.
- DOC TESTS nos comentários rustdoc. Exercitados por
  `cargo test --doc`. Blocos de código em `///` e `//!` são
  compilados e rodados.
- TESTES DE INTEGRAÇÃO sob `tests/integration/`. Superfície
  cross-crate, podem precisar de rede ou wiremock. Exercitados
  por `cargo test --test <nome>`.
- BENCHMARKS sob `benches/`. Micro-benchmarks baseados em
  Criterion para o hot path. Exercitados por
  `cargo bench --bench cache_bench`.

A divisão existe porque os modos de falha são diferentes. Um
teste unitário que bate na rede é uma flake esperando para
acontecer. Um teste de integração que vai no crate público e
roda em toda invocação de `cargo test` é um gargalo de CI.
Categorizar torna o trade-off explícito.

## Categorias de Testes

Sete testes de integração vivem sob `tests/integration/`:

| Teste | Propósito | Rede? | Roda por padrão? |
|---|---|---|---|
| `corpus` | Smoke test em um corpus de URLs reais do YouTube | Sim | Não — `--include-ignored` |
| `rss` | Impõe o budget RSS do NFR-002 de 100 MiB | Não | Sim |
| `offline_cache` | Round-trip de cache hit sem rede (NFR-005) | Não | Sim |
| `provider_a_wiremock` | Provider A contra mocks `wiremock` | Não (mock) | Sim |
| `provider_b_wiremock` | Provider B contra mocks `wiremock` | Não (mock) | Sim |
| `signal_handler_stress` | `SIGINT` / `SIGTERM` sob stress | Não | Não — `--include-ignored` |
| `cli_probing` | Flags da CLI e exit codes | Não | Sim |

Os dois testes com `--include-ignored` são gateados no CI:

- `corpus` — roda no job `test` com `continue-on-error: true`
  porque URLs reais do YouTube podem rate-limit ou mudar de forma.
- `signal_handler_stress` — roda apenas em runners Linux porque
  a semântica de entrega de sinal difere em macOS e Windows.

## Como Rodar

### Testes unitários

```bash
cargo test --lib
```

Roda todo módulo `#[cfg(test)]`. Deve terminar em menos de 30
segundos com cache quente.

### Doc tests

```bash
cargo test --doc
```

Roda todo bloco de código no rustdoc. Deve terminar em menos de
60 segundos com cache quente.

### Testes de integração, um por um

```bash
cargo test --test corpus
cargo test --test rss
cargo test --test offline_cache
cargo test --test provider_a_wiremock
cargo test --test provider_b_wiremock
cargo test --test signal_handler_stress
cargo test --test cli_probing
```

### Testes de integração, todos de uma vez

```bash
cargo test --tests
```

Pula doctests e testes lib. Útil ao iterar em uma única suíte
de integração.

### Testes de integração, incluindo os gateados

```bash
cargo test --test corpus -- --include-ignored
cargo test --test signal_handler_stress -- --include-ignored
```

A flag `--include-ignored` é a forma padrão de `cargo test` para
rodar casos gateados com `#[ignore]`. O CI faz isso para o teste
`corpus` no job `test`.

### Benchmarks

```bash
cargo bench --bench cache_bench
```

Três micro-benchmarks Criterion: cache key composer, URL length
check, BCP 47 locale parser. O CI verifica que o alvo compila
via `cargo bench --no-run`; o bench completo roda apenas sob
demanda.

## Perfis de CI

O arquivo `.github/workflows/ci.yml` roda doze jobs. Cada job
mapeia para um portão de qualidade específico:

| Job | Perfil | Hardware | O que faz |
|---|---|---|---|
| `test` | matriz stable + beta | `ubuntu-latest` | fmt, clippy, build, unit, doc, integration gated corpus, RSS gate, offline cache, wiremock, example smoke, binary size, --help/--version |
| `cross-compile` | 6 alvos | `ubuntu-latest` | `cargo build --release --target <triple>` |
| `publish-dry-run` | stable | `ubuntu-latest` | `cargo package --list` + `cargo publish --dry-run` |
| `msrv` | rustc 1.96.0 | `ubuntu-latest` | `cargo build --locked` + `cargo test --lib --locked` |
| `deny` | stable | `ubuntu-latest` | `cargo deny check` (licenças, bans, advisories) |
| `audit` | stable | `ubuntu-latest` | `cargo audit` para vulnerabilidades conhecidas |
| `public-api` | stable | `ubuntu-latest` | `cargo public-api` baseline + sigilo gate + diff de PR |
| `semver-checks` | stable | `ubuntu-latest` | `cargo semver-checks --all-features` |
| `cargo-install` | stable | `ubuntu-latest` | `cargo install --path` + --version + --help |
| `matrix-os` | stable em 3 SOs | `ubuntu-latest`, `macos-latest`, `windows-latest` | clippy + build por SO |
| `nightly` | nightly | `ubuntu-latest` | clippy + doc build no toolchain instável |
| `docs-link-check` | stable | `ubuntu-latest` | `cargo doc` + `lychee --offline target/doc/` |

O job `matrix-os` é o único que exercita Apple silicon real e
Windows real; o resto da matriz é baseado em Linux.

## Variáveis de Ambiente

A suíte de testes respeita o ambiente padrão de teste Rust mais
algumas chaves específicas do projeto:

- `TEST_INTEGRATION` — quando setada em `1`, roda o teste
  `corpus` mesmo em desenvolvimento local. Por padrão o teste
  `corpus` é gateado com `#[ignore]`.
- `RUST_LOG` — env filter do `tracing`. Valores úteis são
  `info`, `youtube_legend_cli=debug` ou `warn` para saída
  silenciosa. A suíte emite eventos tracing estruturados em stderr.
- `RUSTFLAGS` — o CI seta `-D warnings`. Desenvolvimento local
  sem isso é tranquilo; o CI rejeita warnings.
- `RUSTDOCFLAGS` — o CI seta `-D warnings`. Doc tests falham
  em links intra-doc quebrados.
- `CARGO_TERM_COLOR` — `always` no CI; `auto` local.
- `HTTP_PROXY` / `HTTPS_PROXY` — honrados pelo cliente HTTP
  `reqwest`. Útil para capturar o tráfego upstream durante
  debug local dos testes wiremock.
- `WIREMOCK_PRINT_RESPONSES` — setar em `1` despeja as respostas
  do mock server em stderr. Útil quando uma asserção de
  `provider_a_wiremock` está falhando e você precisa ver o
  payload real.
- `YT_LEGEND_CACHE_DIR` — sobrescreve o diretório de cache
  padrão. Os testes de integração setam isso em um `tempdir`
  para nunca poluir o `~/.cache/youtube-legend-cli/` real.
- `YT_LEGEND_NO_NETWORK` — setar em `1` para falhar qualquer
  teste que tente uma conexão de saída. O teste offline cache
  asserta isso.

## Troubleshooting

### Teste `corpus` flaky

O teste `corpus` bate em URLs reais do YouTube. Rate limits,
falhas transitórias de rede e drift de HTML upstream podem
causar falhas espúrias. O CI roda o teste com
`continue-on-error: true` e imprime a falha para triagem.

Se você vir uma falha local, setar `TEST_INTEGRATION=1` para
re-habilitar o teste, e rode com
`RUST_LOG=youtube_legend_cli=trace` para ver a troca HTTP
completa. O comportamento esperado é um de:

- Todas as URLs retornam um corpo de legenda não vazio.
- Uma URL específica retorna `EX_NOINPUT` (`66`) porque o
  YouTube removeu as legendas. Adicione à lista de skip em
  `corpus.rs`.

### `signal_handler_stress` só roda em Linux

Entrega de sinal em macOS e Windows difere o suficiente para
que o teste seja gateado com `#[cfg(target_os = "linux")]`. O
CI roda no runner `ubuntu-latest` no job `test`. Não se alarme
quando `cargo test --test signal_handler_stress` for no-op no
seu Mac.

### `cargo test` no macOS é lento

A feature `rustls` do `reqwest` usa o backend de criptografia
nativo da plataforma. No macOS isso é `Secure Transport`, que é
lento para a primeira conexão. Execuções subsequentes acertam o
connection pool. Não é regressão do `youtube-legend-cli`.

### `cargo test --doc` reporta links intra-doc quebrados

O CI seta `RUSTDOCFLAGS="-D warnings"` então qualquer link
quebrado é falha de build. Para encontrar o ofensor local:

```bash
RUSTDOCFLAGS="-D warnings" cargo test --doc
```

A mensagem de erro aponta para o bloco `///` ofensor. Corrija
o path ou use uma URL relativa.

### `cargo bench` aborta por falta de Criterion

O alvo de benchmark requer `criterion = "0.5"` em
`[dev-dependencies]`. Se você vir um erro de compilação
mencionando `criterion`, rode `cargo build --benches` para
forçar o download da dev dependency.

## Veja Também
## Testes do ProviderYouTubeDirect (v0.3.0)

Categorias adicionadas em v0.3.0:

- `unit_youtube`: testa `player_response`, `decipher`, `ncode`
  e `caption_track` isoladamente contra fixtures.
- `integration_youtube`: dirige o provider end-to-end contra
  snapshots de HTML congelado.
- `integration_srv3`: exercita o parser Srv3/Json3 com fixtures
  em `tests/fixtures/timedtext/*.srv3`.

### Como Rodar

```bash
cargo test --features youtube-direct
```

### Meta de Cobertura

Expectativa: acima de 80 por cento de cobertura de linha nos
módulos novos (`src/provider/youtube/`, `src/parse/srv3.rs`).


- [README](../README.md) — instalação e execução.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — mapa de módulos e
  pipeline de providers.
- [docs/CROSS_PLATFORM.md](CROSS_PLATFORM.md) — seis alvos de
  cross-compile.
- [docs/MIGRATION.md](MIGRATION.md) — mudanças de v0.2.9 para
  v0.3.0.
