---
name: youtube-legend-cli
description: Aciona quando o usuário pede para baixar legendas, captions, transcrições, SRT ou TXT do YouTube a partir de uma URL. Também aciona em menções a substituto do yt-dlp, daniloaguiarbr, youtube-legend-cli, alternativa ao downsub, save subs, raspagem headless de legendas ou download em lote a partir de uma lista de URLs do YouTube. A v0.3.2 usa exclusivamente o provider headless provider-noteey via Chromium controlado por chromiumoxide, com BrowserFetcher que baixa o Chromium automaticamente e patches de stealth anti-fingerprint. Use para invocar a CLI Rust youtube-legend-cli em recuperação de legendas não interativa, programável e envelopada em JSON.
---


## Instalação e Primeira Execução

### OBRIGATÓRIO
- Use `cargo install youtube-legend-cli` em qualquer deploy de produção
- Toolchain Rust versão 1.88 ou mais recente no host para build a partir do source
- Acesso de rede ao YouTube na TCP/443 para fetches headless
- Acesso de rede para o BrowserFetcher baixar o Chromium na primeira execução
- ESPERE download único de Chromium r1585606 para `~/.cache/youtube-legend-cli/browser/`
- DEFINA `$CHROME` apontando para um binário Chromium existente para pular o download

### PROIBIDO
- NUNCA execute `cargo run --release` em loop apertado em scripts de produção
- NUNCA compile o binário a partir do source em CI quando o pré-compilado está disponível
- NUNCA use `cargo install --path .` para instalar fork local sem auditar o diff
- NUNCA assuma que o primeiro fetch é instantâneo quando o Chromium ainda não foi baixado

### Padrão Correto
```bash
cargo install youtube-legend-cli
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > out.txt
```

### Padrão Correto — Chromium Pré-Instalado
```bash
CHROME=/usr/bin/chromium youtube-legend-cli "https://youtu.be/VIDEO" > out.txt
```


## Referência de Flags da CLI

### OBRIGATÓRIO
- PASSE `--json` para qualquer consumidor programático
- `--lang` aceita `en`, `pt`, `es`, `fr`, `de`, `it` normalizados para BCP 47
- `--provider` aceita SOMENTE `auto` (padrão) ou `provider-noteey`
- `--format` aceita `txt` ou `srt`
- `--batch` lê URLs do stdin uma por linha
- `--cache-ttl` aceita horas inteiras positivas para sobrescrever o TTL, padrão 24
- `--no-cache` força leitura fresca ignorando o cache local
- `--config <PATH>` carrega arquivo TOML de configuração externa
- `--no-progress` suprime barras de progresso no stderr
- `--dry-run` valida a entrada sem disparar o fetch headless
- `--yes` assume sim em prompts não interativos
- `--user-agent` sobrescreve o cabeçalho User-Agent HTTP
- `--timeout` aceita segundos inteiros positivos para limite HTTP, padrão 30
- `--verbose` e `--quiet` controlam volume do log no stderr
- `--log-level` aceita `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` aceita `text` ou `json`
- `--color` aceita `auto`, `always`, `never`
- Combine `--json` com `--lang` para envelopes de saída localizados

### PROIBIDO
- NUNCA passe `--provider youtube-direct`, `provider-a`, `provider-b` ou `provider-headless` — REMOVIDOS na v0.3.2
- NUNCA passe `--asr`, `--no-fallback` ou `--headless` — REMOVIDOS na v0.3.2
- NUNCA hardcode hostnames em scripts
- NUNCA passe URL do YouTube como argumento posicional duas vezes
- NUNCA combine `--no-cache` com invalidação explícita de cache

### Padrão Correto
```bash
youtube-legend-cli --json --lang pt "https://youtu.be/abc" | jaq '.body'
```


## Provider Noteey Headless

### OBRIGATÓRIO
- O ÚNICO provider na v0.3.2 é `provider-noteey` em `src/provider/provider_noteey.rs`
- `--provider auto` resolve para `provider-noteey` sem fallback
- O provider controla Chromium headless via crate `chromiumoxide` 0.9.1
- O BrowserFetcher baixa Chromium r1585606 para `~/.cache/youtube-legend-cli/browser/`
- O download usa single-flight para evitar tempestades de fetch concorrentes
- A env var `$CHROME` sobrescreve a busca do binário e pula o download
- O módulo `src/provider/stealth.rs` aplica patches anti-fingerprint antes da navegação
- Os patches de stealth cobrem `navigator.webdriver`, `plugins`, `languages`, vendor WebGL e `chrome.runtime`
- ESPERE latência maior que provedores HTTP por inicializar um browser real

### PROIBIDO
- NUNCA referencie `youtube-direct`, `provider-a`, `provider-b` ou `provider-headless` — não existem mais
- NUNCA invoque `youtube-direct-probe` — o binário foi removido
- NUNCA persista o Chromium baixado fora do diretório XDG do cache
- NUNCA desabilite os patches de stealth em produção

### Padrão Correto
```bash
youtube-legend-cli --provider provider-noteey --lang pt \
  "https://youtu.be/VIDEO" > legenda.srt
```


## Envelope JSON e Schema

### OBRIGATÓRIO
- VALIDE o campo `error` no stdout antes de confiar no body
- O envelope expõe `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url`, `body`, `error`
- LEIA o campo `error` para detectar falha antes de consumir `body`
- Faça piping do stdout via `jaq` ou parser JSON equivalente
- LEIA o campo `retry_after_seconds` quando presente no envelope de erro

### PROIBIDO
- NUNCA faça parse do stdout linha-a-linha como texto de legenda cru quando `--json` está ativo
- NUNCA pule a checagem do envelope
- NUNCA assuma que o body é sempre uma string preenchida

### Padrão Correto
```bash
out=$(youtube-legend-cli --json "$url")
echo "$out" | jaq -e '.error == null' >/dev/null || echo "$out" | jaq '.error'
```


## Códigos de Saída e sysexits.h

### OBRIGATÓRIO
- `0` para sucesso
- `2` rejeição do parser clap em flag malformada
- `64` EX_USAGE em uso inválido (`InvalidUsage`)
- `65` EX_DATAERR em URL ou entrada inválida (`InvalidUrl`, `InvalidInput`)
- `66` EX_NOINPUT quando a URL não tem legendas disponíveis (`NoSubtitle`)
- `69` EX_UNAVAILABLE em `ProviderUnavailable`, `RateLimited`, `BrowserNotFound`, `CaptchaChallenge`
- `70` EX_SOFTWARE em falha interna (`Internal`, `Timeout`, `Http`, `Io`, `Crypto`, etc)
- `78` EX_CONFIG em erro de configuração (`ConfigError`)
- `130` SIGINT ou SIGTERM em interrupção do usuário

### PROIBIDO
- NUNCA dependa dos números exatos de exit sem o mapeamento por categoria
- NUNCA mascare o exit code com fallback `|| true`
- NUNCA trate `69` como falha permanente — Chromium ausente e captcha são recuperáveis

### Padrão Correto
```bash
youtube-legend-cli "$url" || case $? in
  66) echo "sem legendas" ;;
  69) echo "provider indisponivel, browser ausente ou captcha" ;;
  70) echo "falha interna" ;;
  78) echo "config invalida" ;;
  *) echo "outra falha" ;;
esac
```


## Comportamento de Cache

### OBRIGATÓRIO
- TTL padrão de 24 horas em disco em `~/.cache/youtube-legend-cli/`
- O Chromium baixado vive em `~/.cache/youtube-legend-cli/browser/`
- USE `--no-cache` para fetches frescos em pipelines de auditoria
- USE `--cache-ttl` para sobrescrever o TTL em horas inteiras
- Invalide uma entrada removendo o diretório dela
- O download do browser usa single-flight para evitar tempestades de download

### PROIBIDO
- NUNCA hardcode paths em `/tmp` para armazenamento de cache
- NUNCA delete o diretório inteiro de cache em scripts de produção
- NUNCA redirecione o cache para fora do XDG

### Padrão Correto
```bash
# Invalida uma entrada de legenda
rm -rf ~/.cache/youtube-legend-cli/<autor>/subtitles/<video>/

# Sobrescreve o TTL para um batch de longa duracao
youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"
```


## Retry e Rate Limiting

### OBRIGATÓRIO
- LEIA o campo `retry_after_seconds` do envelope JSON
- PARE de tentar após a janela de delay fornecida pelo envelope
- O fallback interno é 60 segundos com teto de 300 segundos
- TRATE `CaptchaChallenge` como rate-limit suave e aguarde antes de retentar

### PROIBIDO
- NUNCA rode loops de retry client-side sem backoff
- NUNCA martele o provider após resposta de rate-limit
- NUNCA fixe `--timeout` abaixo de 5 segundos

### Padrão Correto
```bash
# erros rate-limited carregam retry_after_seconds no envelope JSON
sleep "$(echo "$out" | jaq '.retry_after_seconds // 60')"
```


## Contratos de Streaming

### OBRIGATÓRIO
- stdout carrega texto de legenda, SRT ou envelope JSON apenas
- stderr carrega logs, progresso e diagnósticos
- DESCARTE o stderr antes de pipar o stdout em `jaq`

### PROIBIDO
- NUNCA faça parse de logs do stderr como se fossem o body
- NUNCA redirecione stderr para arquivo e depois releia como JSON

### Padrão Correto
```bash
youtube-legend-cli --json "$url" 2>/dev/null | jaq '.body'
```


## Tratamento de Erros

### OBRIGATÓRIO
- RAMIFIQUE na categoria `AppError` do envelope
- MAPEIE categorias para política de retry na camada de orquestração
- O enum `AppError` é `#[non_exhaustive]`; trate cada variante como categoria
- USE o helper `reason()` para extrair `NoSubtitleReason` quando o erro for `NoSubtitle`
- TRATE `BrowserNotFound` definindo `$CHROME` ou permitindo o download do BrowserFetcher

### PROIBIDO
- NUNCA entre em panic na lógica de pipeline em exit não-zero
- NUNCA transforme o erro em string para casar por substring

### Padrão Correto
```rust
match err {
    AppError::NoSubtitle(reason) => log::warn!("sem legenda: {reason}"),
    AppError::BrowserNotFound(msg) => log::error!("Chromium ausente: {msg}; defina $CHROME ou habilite o BrowserFetcher"),
    AppError::CaptchaChallenge { .. } => log::warn!("captcha detectado; aguarde antes de retentar"),
    AppError::RateLimited { retry_after_secs } => {
        tokio::time::sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))).await;
    }
    _ => return Err(err),
}
```


## Variáveis de Ambiente

### OBRIGATÓRIO
- `$CHROME` aponta para um binário Chromium existente e pula o download do BrowserFetcher
- `$YT_LEGEND_NO_NETWORK` presente, com qualquer valor, curto-circuita o provider e retorna `ProviderUnavailable`
- `YT_LOG_LEVEL` vence `--log-level`
- `YT_LOG_FORMAT` vence `--log-format`
- `YT_LEGEND_CACHE_DIR` sobrescreve o diretório de cache XDG padrão
- USE a família `YT_*` para qualquer override de configuração

### PROIBIDO
- NUNCA defina `RUST_LOG` diretamente
- NUNCA passe flags de log e env vars que conflitem
- NUNCA confie em `RUST_LOG` para vencer as env vars `YT_*`

### Padrão Correto
```bash
YT_LOG_LEVEL=debug YT_LOG_FORMAT=json youtube-legend-cli "$url"
```

### Padrão Correto — Modo Offline
```bash
YT_LEGEND_NO_NETWORK=1 youtube-legend-cli "$url"
# Retorna ProviderUnavailable com exit 69
```


## Download em Lote

### OBRIGATÓRIO
- USE `--batch` para ler URLs do stdin uma por linha
- COMBINE `--batch` com `--json` para envelopes processáveis por linha
- ENCAPSULE o lote com `timeout` em segundos para evitar travamento
- ESPERE reuso da instância de browser entre URLs do mesmo lote

### PROIBIDO
- NUNCA dispare um processo separado por URL quando `--batch` resolve
- NUNCA pipe stderr para o parser JSON no modo lote

### Padrão Correto
```bash
printf '%s\n' \
  "https://youtu.be/aaa" \
  "https://youtu.be/bbb" \
  | youtube-legend-cli --batch --json --lang pt 2>/dev/null \
  | jaq -r 'select(.error == null) | .body'
```


## Alvos de Cross-Compile

### OBRIGATÓRIO
- `x86_64-unknown-linux-gnu` é o alvo primário de desenvolvimento
- `x86_64-unknown-linux-musl` e `aarch64-unknown-linux-musl` suportam contêineres estáticos
- `x86_64-pc-windows-msvc` cobre Windows nativo
- `x86_64-apple-darwin` e `aarch64-apple-darwin` cobrem macOS
- VERIFIQUE que o Chromium correspondente ao alvo está disponível em runtime

### PROIBIDO
- NUNCA publique um release sem os alvos passando no CI
- NUNCA confie em `cargo build` local como substituto do gate de cross-compile

### Padrão Correto
```bash
cargo install cross --locked
cross build --target x86_64-unknown-linux-musl --release
```


## Veja Também
- [CHANGELOG.md](../../CHANGELOG.md) — histórico completo de releases
- [docs/AGENTS.pt-BR.md](../../docs/AGENTS.pt-BR.md) — guia para agentes com tabela de variantes de erro
- [docs/COOKBOOK.pt-BR.md](../../docs/COOKBOOK.pt-BR.md) — receitas práticas para shell, CI e Python
- [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) — diagrama do pipeline e mapa de módulos
- [docs/CROSS_PLATFORM.pt-BR.md](../../docs/CROSS_PLATFORM.pt-BR.md) — receitas de cross-compile e paths XDG
- [gaps.md](../../gaps.md) — registro vivo de problemas conhecidos
