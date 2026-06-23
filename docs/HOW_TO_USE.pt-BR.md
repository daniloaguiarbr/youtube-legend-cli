# Como Usar — youtube-legend-cli

> Execute um comando, receba um arquivo de legenda limpo. Sem daemon, sem prompts, sem telemetria.

[English](docs/HOW_TO_USE.md) | [Português Brasileiro](docs/HOW_TO_USE.pt-BR.md)

Esta página é o passo a passo prático de 60 segundos. Pressupõe que
você tem uma URL do YouTube e quer o texto da legenda no terminal. A
referência de flags, exit codes e cadeia de provedores está em
[`README.md`](README.md) e
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). A superfície de
integração para agentes de IA e CI está em
[`INTEGRATIONS.md`](INTEGRATIONS.md).

## Pré-requisitos

- Um shell Unix-like em Linux, macOS ou Windows 10/11.
- Rust 1.88.0 ou mais recente se você pretende compilar do código-fonte.
- `curl` NÃO é necessário. O CLI fala HTTP por conta própria.
- Sem Python, sem Node, sem serviços de sistema.

## Primeiro Comando em 60 Segundos

O CLI é distribuído como um único binário estático, então os três
passos abaixo levam menos de um minuto em uma conexão quente.

### 1. Instale

Instale a partir do crates.io para o caminho mais limpo.

```bash
cargo install youtube-legend-cli
```

Ou compile a partir de um checkout local.

```bash
cargo install --path .
```

### 2. Execute

Encaminhe uma URL do YouTube para o binário e redirecione `stdout`
para um arquivo. O corpo da legenda aterrissa em `subtitle.txt`;
logs e progresso ficam em `stderr`.

```bash
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > subtitle.txt
```

### 3. Verifique

Faça um spot-check do tamanho do corpo e da primeira linha não vazia.

```bash
wc -c subtitle.txt
head -n 3 subtitle.txt
```

## Comandos Centrais

O CLI segue uma única convenção: o corpo da legenda vai para
`stdout`, todo outro diagnóstico vai para `stderr`, e `stdin` aceita
uma única URL ou um lote. Os cinco comandos abaixo cobrem os casos
do dia a dia.

### URL Única

```bash
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

### Lote a Partir de um Arquivo

Uma URL por linha em `stdin` mais a flag `--batch`. O CLI as lê
sequencialmente e escreve cada corpo de legenda em ordem.

```bash
youtube-legend-cli --batch < urls.txt > subtitles.txt
```

### Envelope JSON

Passe `--json` para trocar o corpo por um envelope estruturado. O
envelope é o contrato do qual agentes downstream e jobs de CI
dependem.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY"
```

### Idioma Customizado

Use qualquer tag BCP 47, incluindo as formas com underscore que
algumas legendas do YouTube publicam.

```bash
youtube-legend-cli --lang pt-BR "https://youtu.be/dQw4w9WgXcQ"
youtube-legend-cli --lang pt_BR.UTF-8 "https://youtu.be/dQw4w9WgXcQ"
```

### Formato Customizado

O formato padrão é `txt` (texto puro). O formato SRT está
indisponível com o `provider-noteey` atual (retorna exit 64).

```bash
youtube-legend-cli --format txt "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

## Configuração

A flag `--config` aponta o CLI para um arquivo TOML com defaults
que a linha de comando de outra forma teria que repetir. Um arquivo
de config típico fixa o idioma, o formato e o TTL do cache.

```toml
# yt-legend.toml
lang = "pt-BR"
format = "txt"
cache_ttl = 48
verbose = false
```

Passe o caminho do arquivo para o CLI aplicar os defaults.

```bash
youtube-legend-cli --config ./yt-legend.toml \
  "https://youtu.be/NvZ4VZ5hooY"
```

Flags de linha de comando sobrescrevem o arquivo. Uma flag ausente
do arquivo mantém seu default embutido.

## Integração Com Agentes de IA

O CLI é projetado para ser spawnado como subprocesso. Os exemplos
abaixo mostram os três padrões que aparecem com mais frequência em
transcripts de agentes.

### Padrão Um — Encaminhar JSON para o jq

O envelope `--json` é um contrato estável. Um agente capaz de
spawnar um subprocesso e ler dois streams pode dirigir o workflow
inteiro.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY" \
  | jq -r '.content'
```

### Padrão Dois — Captura de Lote Silenciosa

`--quiet` mantém o transcript do agente limpo enquanto o corpo
ainda chega em `stdout`. `--batch` lê uma URL por linha.

```bash
printf '%s\n' \
  "https://youtu.be/NvZ4VZ5hooY" \
  "https://youtu.be/dQw4w9WgXcQ" \
  | youtube-legend-cli --quiet --batch
```

### Padrão Três — Dry Run a Partir do Cache

`--dry-run` pula I/O de rede e serve somente do cache local. Este é
o safety net que um agente deveria usar quando a mesma URL já foi
resolvida na mesma sessão.

```bash
youtube-legend-cli --dry-run --lang pt \
  "https://youtu.be/NvZ4VZ5hooY"
```

## FAQ de Troubleshooting

### Por que o CLI sai com código 66?

O código de saída 66 (`EX_NOINPUT`) significa que não existe trilha
de legenda para o idioma solicitado no vídeo. Tente outro idioma,
ou rode com `--verbose` para confirmar qual trilha o upstream
retornou.

### Por que o CLI sai com código 69 em um vídeo conhecido?

O código de saída 69 (`EX_UNAVAILABLE`) significa que o provedor
retornou uma falha não recuperável. As causas comuns são rate
limiting (`HTTP 429` com `Retry-After` esgotado), Chrome/Chromium
não encontrado, um desafio CAPTCHA, ou uma queda do upstream.
Aguarde alguns minutos e reexecute com `--no-cache` para contornar
qualquer cache negativo obsoleto. Se o Chrome estiver ausente,
defina `$CHROME` ou permita que o `BrowserFetcher` o baixe
automaticamente.

### Onde o cache fica armazenado?

O cache local vive em `~/.cache/youtube-legend-cli/`, indexado por
`(video_id, language, format)`. O TTL padrão é 24 horas. Limpe-o
com `rm -rf ~/.cache/youtube-legend-cli/` se precisar de um fetch a
frio.

### Como faço para logar em JSON em CI?

Defina `YT_LOG_FORMAT=json` no ambiente. O inicializador do
`tracing-subscriber` lê `EnvFilter` primeiro, então a env var
vence sobre a flag `--log-format`.

```bash
YT_LOG_FORMAT=json youtube-legend-cli --json \
  "https://youtu.be/NvZ4VZ5hooY"
```
