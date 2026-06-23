[English](README.md) | [Português Brasileiro](README.pt-BR.md)
# JSON Schemas Index

> Machine-readable contracts for every CLI command and NDJSON event surface.

## English

This directory holds the versioned JSON Schema definitions that the CLI
honors across all of its subcommands. Every subcommand that emits
structured output, every NDJSON event the binary can produce, and every
sidecar file the binary can persist carries a contract here.

### Index

| Schema | Purpose | Companion Subcommand |
|---|---|---|
| `caption-track.schema.json` | Single track entry from `ytInitialPlayerResponse.captions.playerCaptionsTracklistRenderer.captionTracks[]` | legacy `provider-youtube-direct` (removed in v0.3.2) |

### How schemas are versioned

Each schema declares its `$id` rooted at
`https://github.com/daniloaguiarbr/youtube-legend-cli/schemas/<filename>`.
Breaking changes bump the schema filename. Additive changes append a
new `$id` with a versioned path.

## Português Brasileiro

Este diretório contém as definições versionadas de JSON Schema que a CLI
honra em todos os seus subcomandos. Cada subcomando que emite saída
estruturada, cada evento NDJSON que o binário pode produzir, e cada
arquivo sidecar que o binário pode persistir carrega um contrato aqui.

### Índice

| Schema | Propósito | Subcomando Acompanhante |
|---|---|---|
| `caption-track.schema.json` | Entrada única de track do array `ytInitialPlayerResponse.captions.playerCaptionsTracklistRenderer.captionTracks[]` | legacy `provider-youtube-direct` (removido na v0.3.2) |

### Como os schemas são versionados

Cada schema declara seu `$id` enraizado em
`https://github.com/daniloaguiarbr/youtube-legend-cli/schemas/<arquivo>`.
Mudanças que quebram compatibilidade incrementam o nome do arquivo.
Mudanças aditivas anexam um novo `$id` com caminho versionado.
