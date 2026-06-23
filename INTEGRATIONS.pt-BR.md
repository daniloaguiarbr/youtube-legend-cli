# Integrações — youtube-legend-cli

> Encaminhe uma URL do YouTube e receba um arquivo de legenda limpo. Sem daemon, sem prompts, sem telemetria.

[English](INTEGRATIONS.md) | [Português Brasileiro](INTEGRATIONS.pt-BR.md)

Esta página é a superfície de integração para agentes de IA,
orquestradores e pipelines de CI. Documenta quais flags um caller
externo pode usar com segurança, como o ambiente sobrescreve o CLI
e quais flags foram publicadas em qual release. O passo a passo
voltado ao usuário está em [`docs/HOW_TO_USE.md`](docs/HOW_TO_USE.md).

## Agentes e Orquestradores Compatíveis

O CLI é um binário estático único com contrato nativo Unix
`stdin`/`stdout`. Isso significa que qualquer agente capaz de
spawnar um subprocesso e ler dois streams pode dirigi-lo. As
integrações a seguir são as destacadas no README e na matriz de CI.

### Claude Code

Claude Code é a superfície primária de desenvolvimento do mantenedor
e trata `youtube-legend-cli` como ferramenta de subprocesso. A flag
`--json` emite um envelope estável em `stdout` enquanto logs e
progresso ficam em `stderr`, então o agente pode encaminhar o corpo
diretamente para a próxima chamada de ferramenta.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY" \
  | jq '.content'
```

O binário companheiro `snapshot` sonda o provedor de forma
isolada e é o harness que o Claude Code usa para verificar se a
cadeia de provedores v0.2.x ainda retorna corpos de legenda limpos.

### GitHub Actions

A matriz de CI em `ci.yml` já aciona o CLI a partir de um arquivo
de workflow. Fixe a action em uma tag de release específica e
exponha a saída `--json` como artefato de build quando o job
precisa afirmar sobre o formato da resposta.

```yaml
- name: Fetch subtitles
  run: |
    cargo install youtube-legend-cli --locked
    youtube-legend-cli --json "${{ inputs.url }}" > subtitle.json
- name: Verify body length
  run: |
    body_len=$(jq '.content | length' subtitle.json)
    test "$body_len" -gt 0
```

### Aider

Aider pode chamar o CLI através da sua ferramenta de shell. Use
`--batch` com uma URL por linha para que uma única invocação de
subprocesso cubra todas as URLs que o Aider coletou da conversa.

```bash
printf '%s\n' \
  "https://youtu.be/NvZ4VZ5hooY" \
  "https://youtu.be/dQw4w9WgXcQ" \
  | youtube-legend-cli --batch
```

### Continue

Continue roda no VS Code e herda a mesma semântica de shell de
qualquer subprocesso Unix. O cache em
`~/.cache/youtube-legend-cli/` significa que reexecutar a mesma
consulta dentro de uma sessão de editor aberta não toca o provedor
upstream novamente até o TTL expirar.

### Cline

Cline é uma extensão do VS Code que expõe uma ação de shell. O
padrão recomendado é definir `--quiet` para que o transcript do
agente permaneça limpo enquanto o corpo da legenda ainda chega em
`stdout`.

```bash
youtube-legend-cli --quiet \
  "https://youtu.be/NvZ4VZ5hooY" > subtitle.txt
```

### Codex

Codex é o companheiro CLI da OpenAI. Assim como o Aider, ele pode
chamar o binário pela sua ferramenta de shell. A flag `--config`
aceita um arquivo TOML para que uma sessão Codex troque provedores
ou TTL de cache sem redigitar conjuntos longos de flags.

```bash
youtube-legend-cli --config ./yt-legend.toml \
  "https://youtu.be/NvZ4VZ5hooY"
```

## Aliases de Flag

A struct `Cli` derivada do clap expõe 17 flags. Três delas têm
overrides de ambiente companheiros que um orquestrador pode definir
sem modificar a linha de comando do subprocesso.

| Flag | Override de env | Notas |
|------|-----------------|-------|
| `--json` | — | Apenas flag CLI. Emite um envelope estruturado em `stdout`. |
| `--log-level` | `YT_LOG_LEVEL` | `tracing-subscriber` lê `EnvFilter` primeiro, então a env var vence quando definida. |
| `--log-format` | `YT_LOG_FORMAT` | Aceita `text` ou `json`. A env var é a forma canônica de habilitar logs JSON em CI. |

O inicializador do `tracing-subscriber` em `src/logging.rs` é a
fonte autoritativa para precedência de env. Quando uma integração
precisa de formato de log determinístico, defina
`YT_LOG_FORMAT=json` em vez de depender da flag.

## Novas Flags por Versão

A superfície de flags é estável. Cada release anota suas adições em
`CHANGELOG.md`; a tabela abaixo resume as mudanças relevantes para
autores de integração.

| Versão | Novas flags | Notas |
|--------|-------------|-------|
| v0.2.6 | `--config`, `--log-level`, `--log-format`, `--color`, `--no-progress`, `--dry-run`, `--yes` | As sete flags globais foram promovidas no release do playbook do Agent Teams. Toda release anterior já enviava `--lang`, `--format`, `--timeout`, `--verbose`, `--quiet`, `--json`, `--batch`, `--user-agent`, `--cache-ttl`, `--no-cache`. |
| v0.2.7 | — | Sem novas flags. O release corrigiu o slug de categoria do crates.io. |
| v0.2.8 | — | Sem novas flags. O release expôs `secret_endpoints.rs` ao source tree. |
| v0.2.9 | — | Sem novas flags. O release abaixou o MSRV para 1.88.0 em `rust-version`. |
| v0.3.0 | `--provider` | Entrega a flag de seleção de provedor. Desde a v0.3.2, `--provider` aceita apenas `auto` (padrão) e `provider-noteey`. |

## Tabela Resumo

A tabela abaixo é a única página que um agente deveria marcar. Toda
flag que influencia uma integração está aqui, junto com seu
padrão, seu companheiro de ambiente quando existe, e uma descrição
de uma linha do efeito visível ao consumidor.

| Flag | Env | Padrão | Efeito na integração |
|------|-----|--------|----------------------|
| `--config` | — | nenhum | Caminho para um arquivo de config TOML. |
| `--log-level` | `YT_LOG_LEVEL` | `warn` | Verbosidade do tracing. Env vence. |
| `--log-format` | `YT_LOG_FORMAT` | `text` | Formato de log `text` ou `json`. Env vence. |
| `--color` | — | `auto` | Cor sensível a TTY. Defina `never` em CI. |
| `--no-progress` | — | `false` | Suprime barras de progresso em `stderr`. |
| `--dry-run` | — | `false` | Pula I/O de rede; serve somente do cache. |
| `--yes` | — | `false` | Assume sim para qualquer prompt de confirmação. |
| `--lang` | — | `en` | Tag BCP 47, ex. `pt-BR`. |
| `--format` | — | `txt` | `txt` (plano) ou `srt` (preservado). |
| `--timeout` | — | `30` | Timeout HTTP em segundos. |
| `--verbose` | — | `false` | Emite eventos de tracing em `stderr`. |
| `--quiet` | — | `false` | Suprime todo `stderr` não-erro. |
| `--json` | — | `false` | Emite envelope JSON em `stdout`. |
| `--batch` | — | `false` | Lê múltiplas URLs de `stdin`. |
| `--user-agent` | — | nome do crate | Sobrescreve o User-Agent padrão. |
| `--cache-ttl` | — | `24` | TTL do cache em horas. |
| `--no-cache` | — | `false` | Pula leituras do cache. |
| `--provider` | — | `auto` | v0.3.0+. `auto` ou `provider-noteey` (desde v0.3.2). |

A tabela de exit codes segue a convenção BSD `sysexits.h` para que
qualquer orquestrador POSIX possa ramificar por categoria sem
parsear a mensagem legível por humanos. A tabela completa está em
[`README.md`](README.md#exit-codes); a versão curta é `0` para
sucesso, `64` para uso inválido, `65` para URL inválida, `66`
para sem legenda, `69` para upstream indisponível, `70` para erro
interno, `78` para erro de config e `130` para shutdown
cooperativo em `SIGINT`/`SIGTERM`.
