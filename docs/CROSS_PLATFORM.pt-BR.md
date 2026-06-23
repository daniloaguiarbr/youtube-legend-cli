# Guia Multiplataforma — youtube-legend-cli

> Distribuição que simplesmente funciona, na máquina que você usa de verdade.

## A Dor Que Você Já Conhece

Você escreveu um pipeline shell de cinco linhas que baixa a
transcrição de um vídeo no seu notebook Linux. Agora alguém no
macOS tenta o mesmo pipeline e `cargo install` engasga em
`aws-lc-sys`. O check verde do CI não te salvou. Um colega no
Windows 11 recebe `LINK : fatal error LNK1181: cannot open input
file 'crypt32.lib'`. O agente rodando em um contêiner Alpine
dentro de uma imagem distroless não tem `glibc` e o binário
estático que você mandou está dando segfault antes mesmo de
conversar com a rede.

`youtube-legend-cli` foi desenhado para encerrar esses dias. Um
crate, seis alvos de cross-compile, a mesma CLI de 17 flags em
toda plataforma, o mesmo layout de `~/.cache/youtube-legend-cli/`,
e uma única fonte de verdade (`Cargo.toml`
`[package.metadata.docs.rs]`) que o docs.rs renderiza para os
mesmos seis alvos. Esta página é o guia de campo.

## Matriz de Suporte

| SO | Triplo de alvo | Status no CI | Dependências de runtime |
|---|---|---|---|
| Linux x86_64 (glibc) | `x86_64-unknown-linux-gnu` | Verde no `ubuntu-latest` | `glibc >= 2.31` |
| Linux x86_64 (musl) | `x86_64-unknown-linux-musl` | Verde no `ubuntu-latest` | Nenhuma (estático) |
| Linux ARM64 (musl) | `aarch64-unknown-linux-musl` | Verde no `ubuntu-latest` | Nenhuma (estático) |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Verde no job `cross-compile` | `Microsoft Visual C++ 2015-2022 Redistributable` |
| macOS x86_64 (Intel) | `x86_64-apple-darwin` | `continue-on-error: true` (precisa de `osxcross`) | macOS 10.15+ |
| macOS ARM64 (Apple Silicon) | `aarch64-apple-darwin` | `continue-on-error: true` (precisa de `osxcross`) | macOS 11.0+ |

Os três primeiros alvos rodam sem alterações no job `cross-compile`
do `.github/workflows/ci.yml`. Os alvos Apple são cross-compilados
em runners Linux via `osxcross`; a entrada da matriz define
`continue-on-error: true` para que um `osxcross` quebrado nunca
trave o resto do CI. O Apple silicon real é coberto pelo
`matrix-os` rodando em `macos-latest`.

## Linux — glibc vs musl

- O runner Ubuntu padrão produz um binário linkado contra
  `glibc 2.39`. Ele roda em qualquer distribuição com
  `glibc >= 2.31` (Debian 11, RHEL 8.4, Ubuntu 20.04, Alpine 3.13 flavor glibc).
- Os alvos `*-musl` produzem um binário totalmente estático que
  roda em qualquer Linux, inclusive contêineres scratch, imagens
  distroless e roteadores. Escolha musl quando o destino do
  deploy é desconhecido.
- musl não tem `getaddrinfo_a`; a resolução DNS é sequencial.
  Irrelevante para esta CLI (um único hostname, uma requisição
  por vez), mas vale registrar se você embutir o crate.

## macOS — Intel e Apple Silicon

- O job `matrix-os` roda a suíte completa de testes em
  `macos-latest` (atualmente Apple Silicon) e produz um binário
  que usa os caminhos Apple silicon `dyld` e `SecTrust`. O
  binário Intel é cross-compilado em `cross-compile` via
  `osxcross` e é publicado para usuários em hardware mais antigo.
- `reqwest` está configurado com `rustls` (sem `native-tls`),
  então a validação de certificados não depende do keychain do
  macOS. As `webpki-roots` empacotadas são usadas em todos os
  handshakes TLS.
- Sem assinatura de código, sem notarização, sem `xcrun altool`.
  A distribuição é `cargo install` ou `brew install daniloaguiarbr/tap/youtube-legend-cli`.

## Windows — toolchain MSVC

- Apenas o alvo `x86_64-pc-windows-msvc` é suportado. O alvo
  `x86_64-pc-windows-gnu` não é compilado e não está na lista
  `[package.metadata.docs.rs].targets`.
- O usuário final precisa do Microsoft Visual C++ 2015-2022
  Redistributable. A maioria das máquinas Windows 10/11 modernas
  já o tem. Se você empacotar um instalador, o bundle WiX pode
  ser pré-requisito.
- `reqwest` usa `rustls` no Windows. Sem `schannel`, sem a
  certificate store do Windows.
- O tratamento de paths passa pelo crate `directories`; o cache
  mora em `%LOCALAPPDATA%\youtube-legend-cli\cache\`.
- O tratamento de sinal para `Ctrl+C` passa pelo mesmo
  `tokio_util::CancellationToken` usado em Unix; o teste de
  integração `signal_handler_stress` exercita os dois caminhos.

## Contêineres

### Scratch + musl (imagem menor)

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
RUN cargo install youtube-legend-cli --locked --root /out

FROM scratch
COPY --from=builder /out/bin/youtube-legend-cli /youtube-legend-cli
ENTRYPOINT ["/youtube-legend-cli"]
```

Imagem resultante: ~12 MB. Sem shell, sem libc, sem gerenciador de pacotes.

### Distroless + glibc

```dockerfile
FROM rust:1.88-slim AS builder
RUN cargo install youtube-legend-cli --locked --root /out

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /out/bin/youtube-legend-cli /usr/local/bin/
ENTRYPOINT ["youtube-legend-cli"]
```

Imagem resultante: ~30 MB. Inclui `glibc`, `libgcc`, `libstdc++`,
sem shell.

### Alpine

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
RUN cargo install youtube-legend-cli --locked --root /out

FROM alpine:3.20
RUN apk add --no-cache chromium
COPY --from=builder /out/bin/youtube-legend-cli /usr/local/bin/
ENV CHROME=/usr/bin/chromium-browser
ENTRYPOINT ["youtube-legend-cli"]
```

A CLI requer Chrome/Chromium em runtime para o provider
`provider-noteey`. Defina `$CHROME` apontando para o binário do
browser, ou deixe o `BrowserFetcher` baixar automaticamente.

## Suporte a Shells

A CLI é um contrato puro de stdin/stdout; o shell é a camada de
orquestração. Os cinco shells principais são exercitados no
docs link-check e na matriz de build de exemplos:

- `bash` 4+ em Linux/macOS, 5+ em Windows via Git Bash ou WSL.
- `zsh` 5+ (padrão em macOS moderno).
- `fish` 3+.
- `elvish` 0.18+ (smoke-tested via `examples/batch`).
- `powershell` 7+ em Windows e PowerShell Core em Linux/macOS.

Scripts de completion podem ser gerados via introspecção
`clap` `--help`; o README envia um bloco copiável para cada shell.

## Paths de Arquivos e XDG

- Linux: `$XDG_CACHE_HOME/youtube-legend-cli/cache/` (padrão
  `~/.cache/youtube-legend-cli/cache/`). Arquivo de config em
  `$XDG_CONFIG_HOME/youtube-legend-cli/config.toml`.
- macOS: `~/Library/Caches/youtube-legend-cli/cache/`. Config em
  `~/Library/Application Support/youtube-legend-cli/config.toml`.
- Windows: `%LOCALAPPDATA%\youtube-legend-cli\cache\`. Config em
  `%APPDATA%\youtube-legend-cli\config.toml`.
- O cache é chaveado por TTL, padrão 24 horas (`--cache-ttl <HOURS>`).
  Use `--no-cache` para pular leituras; use `--cache-ttl 0` para
  desabilitar apenas escritas.

O path é resolvido via crate `directories`, que honra
`XDG_CACHE_HOME` e `HOME` em Unix e `LOCALAPPDATA` em Windows.

## Performance por Alvo

Wall-clock para um único `cargo build --release` da CLI no
hardware do CI (cache quente, 4 núcleos):

| Alvo | Tempo de build | Tamanho do binário |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ~3 min | 8,4 MB stripped |
| `x86_64-unknown-linux-musl` | ~3 min 10 s | 8,5 MB stripped |
| `aarch64-unknown-linux-musl` | ~6 min (QEMU) | 8,3 MB stripped |
| `x86_64-pc-windows-msvc` | ~4 min | 8,7 MB stripped |
| `x86_64-apple-darwin` | `continue-on-error` | ~9,1 MB |
| `aarch64-apple-darwin` | `continue-on-error` | ~9,0 MB |

Strip + `lto = "thin"` está habilitado no perfil release
(`Cargo.toml` `[profile.release]`). O step `Verify binary size`
do CI falha o build se o binário exceder 20 MB (NFR-003).

## Agentes Validados por Plataforma

| Agente | Linux | macOS | Windows | Notas |
|---|---|---|---|---|
| `claude -p` (Claude Code) | Sim | Sim | Sim | Apenas OAuth desde v0.x do runner |
| `codex exec` (OpenAI Codex) | Sim | Sim | Sim | Requer CLI `codex` no `PATH` |
| Aider | Sim | Sim | Sim | Usa o envelope `--json` para streaming |
| Continue.dev | Sim | Sim | Sim | Plugin suporta o shape `commands` |
| Goose | Sim | Sim | Parcial | Pipe-friendly via stdin/stdout |
| Loop shell puro | Sim | Sim | Sim (Git Bash) | Sem agente; puro estilo `curl` |

## Veja Também

- [README](../README.md) — instalação, flags, exit codes.
- [docs/MIGRATION.md](MIGRATION.md) — o que mudou em v0.2.9 / v0.3.0.
- [docs/TESTING.md](TESTING.md) — como os seis alvos são exercitados no CI.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — mapa de módulos e pipeline de providers.
