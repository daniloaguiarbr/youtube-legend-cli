[English](MIGRATION.md) | [Português Brasileiro](MIGRATION.pt-BR.md)
# Guia de Migração — youtube-legend-cli

> Notas de atualização para cada release major.

## v0.3.3 — Correções de Qualidade e Precisão

v0.3.3 corrige 10 bugs encontrados durante auditoria end-to-end.
Sem breaking changes; todas as correções são aditivas ou corretivas.

| Correção | Impacto |
|---|---|
| Envelope JSON de erro para falhas pre-fetch (GAP-060) | `--json` agora emite erros estruturados para falhas de validação |
| Campo `language_detected` (GAP-061) | Novo campo booleano no envelope JSON; `false` quando provider não seleciona idioma |
| Limpeza de marcadores de speaker `>>` (GAP-062) | Parser remove prefixos `>>` das linhas de transcrição |
| Precisão de `byte_size` (GAP-065) | Agora reflete o tamanho do conteúdo limpo NFC, não do HTML cru |
| Flag `--verbose` funcional (GAP-066) | Era uma flag morta; agora ativa logging nível INFO |
| Ruído de cleanup do Chromium (GAP-067) | Sem mais `kill signal failed` no stderr |
| Limitação de SRT no help (GAP-068) | `--help` documenta que SRT não está disponível com provider-noteey |
| Saída NDJSON em batch (GAP-069) | `--batch --json` agora emite objetos JSON terminados por newline |

### Passos de Migração

1. Se você parseia saída `--json`, adicione tratamento para o novo
   campo booleano `language_detected`.
2. Se você depende de `byte_size`, note que agora corresponde
   exatamente ao tamanho do campo `content` (antes podia diferir).
3. Nenhuma outra mudança necessária.

## v0.3.2 — Consolidação em Provider Único

v0.3.2 remove todos os providers exceto `provider-noteey`. Esta é
uma breaking change para scripts que pinam um provider específico.

| Removido | Substituto |
|---|---|
| `--provider youtube-direct` | `--provider auto` (resolve para `provider-noteey`) |
| `--provider provider-a` | `--provider auto` |
| `--provider provider-b` | `--provider auto` |
| `--provider provider-headless` | `--provider auto` |
| Flag `--asr` | removida, sem substituto |
| Flag `--no-fallback` | removida, sem substituto |
| Flag `--headless` | removida, sem substituto |
| Binário `youtube-direct-probe` | removido, sem substituto |

### Passos de Migração

1. Remova qualquer `--provider provider-a`, `--provider provider-b`,
   `--provider youtube-direct` ou `--provider provider-headless`
   dos seus scripts. Use `--provider auto` ou omita a flag.
2. Remova as flags `--asr`, `--no-fallback` e `--headless`.
3. Garanta que Chrome/Chromium está disponível, ou deixe o
   `BrowserFetcher` baixar automaticamente. Defina `$CHROME`
   para sobrescrever o caminho do binário.
4. O campo `body` do envelope JSON foi renomeado para `content`.
   Atualize filtros `jq`: `.body` → `.content`.

## v0.3.0 — Provider YouTube-Direct

A release v0.3.0 adiciona um provider YouTube-direct de primeira
classe e três novas flags. O comportamento padrão para usuários
que nunca setam uma flag é preservado: a CLI continua falando com
os mesmos providers terceiros na mesma ordem. As novas flags e o
novo provider se encaixam no pipeline existente como uma camada
opt-in.

| Área | v0.2.9 | v0.3.0 |
|---|---|---|
| Providers na cadeia | ProviderA e depois ProviderB | YouTube-direct e depois ProviderA e depois ProviderB |
| Flag `--provider` | ausente | `auto`, `youtube-direct`, `provider-a`, `provider-b`, `provider-headless` |
| Flag `--asr` | ausente | `bool`, válido apenas com `youtube-direct` |
| Flag `--no-fallback` | ausente | `bool`, válido apenas com `--provider auto` |
| Comportamento de `--dry-run` | serve do cache | serve do cache, também pula YouTube-direct quando setada |
| Binários enviados | `youtube-legend-cli`, `snapshot` | `youtube-legend-cli`, `snapshot`, `youtube-direct-probe` |
| Envelope JSON | inalterado | inalterado (campos apenas aditivos) |
| Exit codes | BSD `sysexits.h` (64-78) | BSD `sysexits.h` (64-78) |
| MSRV | `1.88.0` | `1.88.0` |

A trait `Provider` e as implementações concretas `ProviderA` e
`ProviderB` estão intocadas. Embarcadores que consomem este
crate como biblioteca não precisam recompilar o código.

## Migração Passo a Passo

1. Atualize o binário. `cargo install youtube-legend-cli --locked --force`.
2. Verifique a instalação. `youtube-legend-cli --version` reporta
   `0.3.0` ou mais novo.
3. Smoke-test do comportamento padrão. Pipe uma URL conhecida
   pela `youtube-legend-cli`; a saída deve ser byte-idêntica à
   de v0.2.9 para a mesma entrada.
4. Audite seus scripts em busca de regressões de flag. As 17
   flags que você conhecia em v0.2.9 estão presentes e se
   comportam de forma idêntica. Novas flags (`--provider`,
   `--asr`, `--no-fallback`) são aditivas e não mudam padrões.
5. Audite seus scripts em busca de novas capacidades. A cadeia
   `auto` agora começa com YouTube-direct. Se você tem a
   expectativa hard-coded de que "a primeira chamada de rede
   acerta o provider A", pine explicitamente:
   `--provider provider-a`.
6. Teste consumidores JSON. O envelope é apenas aditivo; filtros
   `jq` existentes continuam funcionando. Novos campos sob
   `meta.provider` podem aparecer; parsers defensivos devem
   ignorar desconhecidos.
7. Se você embarca a biblioteca, linke contra o novo re-export
   `Provider`. O struct `ProviderYouTubeDirect` é acessível via
   `youtube_legend_cli::provider::ProviderYouTubeDirect`. É
   `pub` mas a superfície pública da trait não mudou.
8. Faça rollout atrás de uma flag. Para deploy em fleet, envie
   v0.3.0 com `--provider auto` e monitore as métricas. O gate
   `dry_run` na nova camada é uma rede de segurança.

## Mudanças no Schema JSON

O envelope `--json` mantém o mesmo formato de v0.2.9 com
campos aditivos. Um envelope mínimo (v0.2.9) se parece com:

```json
{
  "url": "https://youtu.be/dQw4w9WgXcQ",
  "video_id": "dQw4w9WgXcQ",
  "language": "en",
  "format": "txt",
  "provider": "provider-a",
  "body": "...",
  "cached": false,
  "elapsed_ms": 1234
}
```

Um envelope v0.3.0 com a camada YouTube-direct selecionada adiciona:

```json
{
  "url": "...",
  "video_id": "...",
  "language": "en",
  "format": "txt",
  "provider": "youtube-direct",
  "body": "...",
  "cached": false,
  "elapsed_ms": 987,
  "meta": {
    "provider": "youtube-direct",
    "captions_url": "https://www.youtube.com/api/timedtext?...",
    "deciphered_signature": "<redigido>"
  }
}
```

Parsers existentes devem tratar o bloco `meta` como opaco e
continuar usando campos top-level. A `deciphered_signature` é
intencionalmente redigida; consumidores que precisam da signature
crua devem chamar a API do embedder diretamente, não parsear a
saída da CLI.

O schema autoritativo está em
`docs/schemas/caption-track.schema.json`.

## Notas de Compatibilidade

- **BC break em exit codes**: nenhum em v0.3.0. O mapeamento
  BSD `sysexits.h` foi introduzido em v0.2.6 e está preservado.
- **BC break no envelope JSON**: nenhum. Apenas aditivo.
- **BC break em flags de CLI**: nenhum. As 17 flags conectadas
  mantêm suas semânticas. Novas flags (`--provider`, `--asr`,
  `--no-fallback`) são adições puras.
- **BC break na API da biblioteca**: nenhum. A trait `Provider`,
  `ProviderA`, `ProviderB` e `ProviderChain` mantêm sua
  superfície pública. O novo `ProviderYouTubeDirect` é aditivo.
- **BC break no layout do cache**: nenhum. Arquivos de cache são
  compatíveis forward e backward entre v0.2.6 e v0.3.0.
- **BC break em dependências**: `reqwest 0.13` (era 0.12) já
  chegou em v0.2.6; esta release não mexe em majors.

## Rollback

Se um rollout de v0.3.0 se comportar mal, reverta para v0.2.9
em três passos:

1. `cargo install youtube-legend-cli --version 0.2.9 --locked --force`.
2. Restaure seus scripts anteriores. As 17 flags que você tinha
   estão inalteradas; apenas as novas flags deixam de ser
   reconhecidas.
3. Limpe o cache local. v0.3.0 grava um novo campo
   `meta.provider` que v0.2.9 não entende; arquivos de cache
   stale são lidos por ambas as versões, mas o novo campo é
   ignorado por v0.2.9. Nenhuma limpeza manual é necessária.

Pine a versão nos seus scripts com a flag explícita `--version`
na hora da instalação. A CLI não auto-atualiza; o binário no
disco é o binário que roda.

## Veja Também
- [CHANGELOG.md](../CHANGELOG.md) — histórico completo de releases.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — pipeline de providers e
  semântica da cadeia.
- [docs/CROSS_PLATFORM.md](CROSS_PLATFORM.md) — seis alvos de
  cross-compile, receitas de contêiner, paths XDG.
- [docs/TESTING.md](TESTING.md) — como a migração é exercitada
  na suíte de testes de integração.
