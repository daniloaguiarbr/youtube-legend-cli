---
name: youtube-legend-cli
description: Aciona quando o usuário pede para baixar legendas, captions, transcrições, SRT, VTT ou qualquer arquivo de legenda do YouTube a partir de uma URL. Também aciona em menções a substituto do yt-dlp, daniloaguiarbr, youtube-legend-cli, alternativa ao downsub, save subs ou download em lote de legendas a partir de uma lista de URLs do YouTube. Use para invocar a CLI Rust youtube-legend-cli em recuperação de legendas não interativa, programável e envelopada em JSON.
---

## Instalação e Primeira Execução

### OBRIGATÓRIO
- Toolchain Rust versão 1.88 ou mais recente no host
- Acesso de rede ao YouTube na TCP/443 para fetches de primeira execução
- Use `cargo install youtube-legend-cli` em qualquer deploy de produção
- MSRV é 1.88 stable conforme `Cargo.toml`

### PROIBIDO
- Não execute `cargo run --release` em loop apertado em scripts de produção
- Não compile o binário a partir do source em CI quando o pré-compilado está disponível
- Não use `cargo install --path .` para instalar fork local sem auditar o diff

### Padrão Correto
```bash
cargo install youtube-legend-cli
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > out.txt
```

## Referência de Flags da CLI

### OBRIGATÓRIO
- Passe `--json` para qualquer consumidor programático
- `--lang` aceita códigos BCP 47 (`pt-BR`, `en-US`, `es-ES`)
- `--provider` aceita `auto`, `youtube-direct`, `provider-a`, `provider-b` ou `provider-headless`
- `--asr` força a trilha auto-gerada, válido apenas com `youtube-direct`
- `--no-fallback` desabilita a cadeia, válido apenas com `--provider auto`
- `--format` aceita `txt`, `srt`, `vtt` ou `json`
- `--batch` lê URLs do stdin uma por linha
- `--cache-ttl` aceita horas inteiras positivas para sobrescrever o TTL
- `--no-cache` força leitura fresca ignorando o cache local
- `--config <PATH>` carrega arquivo TOML de configuração externa
- `--no-progress` suprime barras de progresso no stderr
- `--yes` assume sim em prompts não interativos
- `--user-agent` sobrescreve o cabeçalho User-Agent HTTP
- `--timeout` aceita segundos inteiros positivos para limite HTTP
- `--verbose` e `--quiet` controlam volume do log no stderr
- `--log-level` aceita `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` aceita `text` ou `json`
- `--color` aceita `auto`, `always`, `never`
- Combine `--json` com `--lang` para envelopes de saída localizados

### PROIBIDO
- Não hardcode hostnames em scripts
- Não passe URL do YouTube como argumento posicional duas vezes
- Não combine `--no-cache` com invalidação explícita de cache
- Não combine `--asr` com `provider-a` ou `provider-b`
- Não combine `--no-fallback` com um provedor fixo

### Padrão Correto
```bash
youtube-legend-cli --json --lang pt-BR "https://youtu.be/abc" | jaq '.body'
```

## Envelope JSON e Schema

### OBRIGATÓRIO
- Valide o campo `error` no stdout antes de confiar no body
- Ramifique no campo `code` que casa com sysexits BSD
- Leia o campo `retry_after_seconds` quando presente
- Piping do stdout via `jaq` ou parser JSON equivalente
- O envelope v0.3.0 adiciona bloco `meta` com `provider`, `captions_url`, `deciphered_signature`
- O campo `deciphered_signature` é intencionalmente redigido no envelope
- Schema autoritativo em `docs/schemas/caption-track.schema.json`

### PROIBIDO
- Não faça parse do stdout linha-a-linha como texto de legenda cru
- Não pule a checagem do envelope
- Não assuma que o body é sempre uma string

### Padrão Correto
```bash
out=$(youtube-legend-cli --json "$url")
echo "$out" | jaq -e '.error == null' >/dev/null || echo "$out" | jaq '.message'
```

## Exit Codes e sysexits.h

### OBRIGATÓRIO
- `0` para sucesso
- `64` EX_USAGE em argumentos inválidos
- `65` EX_DATAERR em resposta upstream malformada
- `66` EX_NOINPUT quando a URL não tem legendas disponíveis
- `69` EX_UNAVAILABLE quando a cadeia de provedores está esgotada
- `70` EX_SOFTWARE em falha interna, incluindo erros do provider YouTube
- `78` EX_CONFIG em erro de configuração
- `130` SIGINT em interrupção do usuário

### PROIBIDO
- Não dependa dos números exatos de exit sem o mapeamento por categoria
- Não mascare o exit code com fallback `|| true`

### Padrão Correto
```bash
youtube-legend-cli "$url" || case $? in
  66) echo "sem legendas" ;;
  69) echo "upstream fora" ;;
  70) echo "falha interna do provider" ;;
  *) echo "outra falha" ;;
esac
```

## Cadeia de Provedores e Seleção

### OBRIGATÓRIO
- Ordem padrão: `youtube-direct`, depois `provider_a`, `provider_b`, e `provider_headless` quando a feature `headless` está habilitada
- O provider `youtube-direct` consulta o endpoint público do YouTube via `ytInitialPlayerResponse` e `captionTracks[].baseUrl`
- A trait `Provider` é pública e instanciada via `provider::ProviderYouTubeDirect` em `src/provider/provider_youtube_direct.rs`
- Fixe um provedor específico apenas para teste determinístico
- Documente qualquer override de provedor no cabeçalho do script

### PROIBIDO
- Não fixe `provider-a` em scripts de CI porque perde o sinal youtube-direct
- Não assuma que um único provedor cobre todo o catálogo
- Não confunda o módulo `src/provider/` com o submódulo `src/provider/youtube/`

### Padrão Correto
```bash
# Produção deixa a cadeia fazer fallback automático
youtube-legend-cli --provider auto "https://youtu.be/VIDEO"

# Debug fixa um provedor e desabilita fallback
youtube-legend-cli --provider youtube-direct --no-fallback "https://youtu.be/VIDEO"
```

## Provider YouTube Direct (v0.3.0)

### OBRIGATÓRIO
- O provider `ProviderYouTubeDirect` vive em `src/provider/provider_youtube_direct.rs`
- Módulos auxiliares em `src/provider/youtube/` incluem `player_response.rs`, `player_js.rs`, `decipher.rs`, `ncode.rs` e `caption_track.rs`
- O parser de resposta do player extrai `ytInitialPlayerResponse` da watch page via regex
- O decipher de signature usa a tabela de operações extraída de `base.js` em cache
- O decipher do parâmetro n usa a função `ncode` para vídeos protegidos
- A conversão de Srv3 e Json3 para SRT acontece em `src/parse/srv3.rs`
- O cache do `base.js` fica em `~/.cache/youtube-legend-cli/player/<versão>.js` com TTL de 7 dias
- A feature `headless` permanece opcional e gateada em build time

### PROIBIDO
- Não invoque o provider YouTube direct sem o módulo `src/provider/youtube/`
- Não persista o `base.js` fora do diretório XDG do cache

### Padrão Correto
```bash
youtube-legend-cli --provider youtube-direct --asr --lang pt-BR \
  "https://youtu.be/VIDEO" > legenda.srt
```

## Comportamento de Cache

### OBRIGATÓRIO
- TTL padrão de 24 horas em disco em `~/.cache/youtube-legend-cli/`
- O cache do player JavaScript vive em `~/.cache/youtube-legend-cli/player/`
- Use `--no-cache` para fetches frescos em pipelines de auditoria
- Use `--cache-ttl` para sobrescrever o TTL em horas inteiras
- Invalide uma entrada removendo o diretório dela
- O cache do player usa single-flight para evitar tempestades de download

### PROIBIDO
- Não hardcode paths em `/tmp` para armazenamento de cache
- Não delete o diretório inteiro de cache em scripts de produção
- Não redirecione o cache para fora do XDG

### Padrão Correto
```bash
# Invalida uma entrada
rm -rf ~/.cache/youtube-legend-cli/<autor>/subtitles/<video>/

# Sobrescreve o TTL para um batch de longa duração
youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"
```

## Retry e Rate Limiting

### OBRIGATÓRIO
- Honre o header `Retry-After` em respostas HTTP 429
- Leia o campo `retry_after_seconds` do envelope JSON
- Pare de tentar após a janela de delay fornecida pelo envelope
- O fallback interno é 60 segundos com teto de 300 segundos
- O throttle de uma requisição por segundo é por cadeia, não por provedor

### PROIBIDO
- Não rode loops de retry client-side sem backoff
- Não martele o mesmo provedor após resposta de rate-limit
- Não fixe `--timeout` abaixo de 5 segundos

### Padrão Correto
```bash
# erros rate-limited carregam retry_after_seconds no envelope JSON
sleep "$(echo "$out" | jaq '.retry_after_seconds')"
```

## Contratos de Streaming

### OBRIGATÓRIO
- stdout carrega texto de legenda, SRT, VTT ou envelope JSON apenas
- stderr carrega logs, progresso e diagnósticos
- Descarte o stderr antes de pipar o stdout em `jaq`

### PROIBIDO
- Não faça parse de logs do stderr como se fossem o body
- Não redirecione stderr para arquivo e depois releia como JSON

### Padrão Correto
```bash
youtube-legend-cli --json "$url" 2>/dev/null | jaq '.body'
```

## Tratamento de Erros

### OBRIGATÓRIO
- Ramifique na categoria `AppError` do envelope
- Mapeie categorias para política de retry na camada de orquestração
- Leia `docs/AGENTS.pt-BR.md` para a tabela completa de categorias
- O enum `AppError` é `#[non_exhaustive]`; trate cada variante como categoria
- Use o helper `reason()` para extrair `NoSubtitleReason` quando o erro for `NoSubtitle`

### PROIBIDO
- Não entre em panic na lógica de pipeline em exit não-zero
- Não assuma uma única categoria de erro para toda a cadeia de provedores
- Não transforme o erro em string para casar por substring

### Padrão Correto
```rust
match err {
    AppError::NoSubtitle(reason) => log::warn!("sem legenda: {reason}"),
    AppError::RateLimited { retry_after_secs } => {
        tokio::time::sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))).await;
    }
    _ => return Err(err),
}
```

## Variáveis de Ambiente

### OBRIGATÓRIO
- `YT_LOG_LEVEL` vence `--log-level`
- `YT_LOG_FORMAT` vence `--log-format`
- `YT_LEGEND_CACHE_DIR` sobrescreve o diretório de cache XDG padrão
- `YT_LEGEND_NO_NETWORK` desabilita todo tráfego de rede para modo offline
- Use a família `YT_*` para qualquer override de configuração

### PROIBIDO
- Não defina `RUST_LOG` diretamente
- Não passe flags de log e env vars que conflitem
- Não confie em `RUST_LOG` para vencer as env vars `YT_*`

### Padrão Correto
```bash
YT_LOG_LEVEL=debug YT_LOG_FORMAT=json youtube-legend-cli "$url"
```

## Capacidades do Provider YouTube Direct (v0.3.0)

### OBRIGATÓRIO
- Passe `--provider youtube-direct` para forçar o provider nativo
- Confie em legendas auto-geradas via `--asr` sem fallback para third-party
- Receba SRT canônico do YouTube convertido de Srv3 e Json3 localmente
- Diagnostique com `youtube-direct-probe <video-id>` quando o decipher falhar
- Aplique filtros em `captionTracks` por `languageCode` e `kind`

### PROIBIDO
- Não invoque `youtube-direct-probe` em pipelines de produção
- Não assuma que a trilha manual existe antes de tentar a auto-gerada

### Padrão Correto
```bash
youtube-direct-probe <video-id> | jaq -r '.signature_status'
```

## Binário de Diagnóstico `youtube-direct-probe`

### OBRIGATÓRIO
- O binário vive em `src/bin/youtube-direct-probe.rs` e é compilado junto com a CLI
- O probe carrega o `base.js` em cache e roda o decipher em uma signature sintética
- O probe imprime um objeto JSON por linha com `signature_status`, `player_js_version`, `cache_hit` e `decipher_error` opcional
- O probe respeita `YT_LEGEND_NO_NETWORK` para diagnóstico offline

### PROIBIDO
- Não invoque o probe em loops de produção
- Não parseie a saída como se fosse o body de legenda

### Padrão Correto
```bash
youtube-direct-probe dQw4w9WgXcQ
```

## Comportamento de Erros (v0.3.0)

### OBRIGATÓRIO
- `SignatureDecipherFailed(String)` retorna exit 70 `EX_SOFTWARE`
- `PlayerResponseMissing(String)` retorna exit 70
- `CaptionTrackNotFound` retorna exit 70
- `TimedtextUpstreamError(String)` retorna exit 70
- As quatro variantes residem em `src/error.rs` e são aditivas ao enum `AppError`
- Clientes da biblioteca continuam funcionando sem recompilação

### PROIBIDO
- Não trate as novas variantes como `EX_UNAVAILABLE`
- Não confunda `SignatureDecipherFailed` com rate limiting

### Padrão Correto
```rust
match err {
    AppError::SignatureDecipherFailed(s) => log::error!("decipher falhou: {s}"),
    AppError::PlayerResponseMissing(s) => log::error!("player response ausente: {s}"),
    AppError::CaptionTrackNotFound => log::warn!("sem trilha de legenda"),
    AppError::TimedtextUpstreamError(s) => log::error!("upstream timedtext falhou: {s}"),
    _ => return Err(err),
}
```

## Marcos do GAP-001 (M1 a M5 + M3.5)

### OBRIGATÓRIO
- M1 implementa o parser de `ytInitialPlayerResponse` em `src/provider/youtube/player_response.rs`
- M2 implementa o fetcher de timedtext sem signature usando `baseUrl` direto
- M3 implementa o signature decipher portado de `base.js` com cache XDG
- M3.5 implementa o decipher do parâmetro n para vídeos protegidos em `src/provider/youtube/ncode.rs`
- M4 integra o provider na cadeia e adiciona as flags `--provider`, `--asr` e `--no-fallback`
- M5 adiciona fixtures de teste em `tests/fixtures/player/` e `tests/fixtures/timedtext/`
- O gate de CI em `.github/workflows/youtube-direct.yml` exige os 6 alvos de cross-compile verdes

### PROIBIDO
- Não pule o gate do CI YouTube direct antes de merge
- Não altere o parser de Srv3 sem atualizar os snapshots de teste

### Padrão Correto
```bash
# Roda os testes do provider YouTube direct
cargo test --test youtube_direct -- --ignored
```

## Alvos de Cross-Compile

### OBRIGATÓRIO
- 6 alvos via job `cross-compile` em `ci.yml`
- `x86_64-unknown-linux-gnu` é o alvo primário de desenvolvimento
- `x86_64-unknown-linux-musl` e `aarch64-unknown-linux-musl` suportam contêineres estáticos
- `x86_64-pc-windows-msvc` cobre Windows nativo
- `x86_64-apple-darwin` e `aarch64-apple-darwin` rodam com `continue-on-error: true` por exigirem osxcross

### PROIBIDO
- Não publique um release sem todos os 6 alvos passando no CI
- Não confie em `cargo build` local como substituto do gate de cross-compile

### Padrão Correto
```bash
cargo install cross --locked
cross build --target x86_64-unknown-linux-musl --release
```

## Veja Também
- [CHANGELOG.md](../../CHANGELOG.md) — histórico completo de releases
- [docs/AGENTS.pt-BR.md](../../docs/AGENTS.pt-BR.md) — guia para agentes com tabela de variantes
- [docs/MIGRATION.pt-BR.md](../../docs/MIGRATION.pt-BR.md) — migração de v0.2.x para v0.3.x
- [docs/COOKBOOK.pt-BR.md](../../docs/COOKBOOK.pt-BR.md) — receitas práticas para shell, CI e Python
- [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) — diagrama do pipeline e mapa de módulos
- [docs/CROSS_PLATFORM.pt-BR.md](../../docs/CROSS_PLATFORM.pt-BR.md) — receitas de cross-compile e paths XDG
- [docs/TESTING.pt-BR.md](../../docs/TESTING.pt-BR.md) — suíte de testes de integração
- [docs/schemas/caption-track.schema.json](../../docs/schemas/caption-track.schema.json) — schema JSON autoritativo
- [gaps.md](../../gaps.md) — registro vivo de problemas conhecidos
