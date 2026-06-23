---
name: youtube-legend-cli
description: Aciona quando o usuário pede para baixar legendas, captions, transcrições, SRT ou TXT do YouTube a partir de uma URL (watch, shorts, embed, youtu.be). Também aciona em menções a substituto do yt-dlp, youtube-legend-cli, alternativa ao downsub, noteey, save subs, download em lote ou extração headless de legendas. Esta skill DEVE ser usada para invocar a CLI Rust youtube-legend-cli em recuperação não interativa e programável de legendas via pipeline headless-Chromium provider-noteey. Cobre todas as 18 flags da CLI com fórmulas prontas, campos do envelope JSON (content, language_detected, byte_size, video_id, duration_ms), exit codes BSD sysexits, saída NDJSON em lote, cache com TTL, retry e rate-limit, BrowserFetcher auto-download, override CHROME, variáveis de ambiente, config TOML e modo offline.
---


# youtube-legend-cli


## Identidade e Arquitetura
- youtube-legend-cli é uma CLI Rust não interativa que baixa legendas do YouTube
- A CLI usa EXATAMENTE UM provider chamado `provider-noteey` (noteey.com via Chromium headless)
- `--provider auto` resolve para `provider-noteey` sem fallback
- noteey.com retorna UMA transcrição por página no idioma original do vídeo sem seleção de idioma
- O parser remove timestamps, marcadores de anotação `[Music]` e marcadores de speaker `>>`
- O parser normaliza toda saída para Unicode NFC
- stdout carrega SOMENTE o texto da legenda ou o envelope `--json`
- stderr carrega SOMENTE logs, barras de progresso e diagnósticos
- SEMPRE descarte stderr antes de pipar stdout em `jaq`
- `--format srt` está INDISPONÍVEL com provider-noteey e retorna exit 64
- Instale com `cargo install youtube-legend-cli` — MSRV é Rust 1.88


## Flags da CLI
- PASSE `--json` para qualquer consumidor programático
- `--lang` aceita ISO 639-1 ou BCP 47 normalizado para subtag primária (`pt-BR`, `pt_BR.UTF-8`, `EN-us` funcionam)
- `--format` aceita `txt` (padrão, timestamps removidos) ou `srt` (INDISPONÍVEL, retorna exit 64)
- `--provider` aceita SOMENTE `auto` (padrão) ou `provider-noteey`
- `--timeout` aceita segundos inteiros positivos (padrão 30)
- `--config <PATH>` carrega arquivo TOML de configuração
- `--cache-ttl` aceita horas inteiras positivas (padrão 24)
- `--no-cache` força leitura fresca ignorando o cache
- `--no-progress` suprime barras de progresso no stderr
- `--dry-run` pula I/O de rede e serve somente do cache
- `--yes` assume sim em prompts não interativos
- `--batch` lê URLs do stdin uma por linha e emite NDJSON quando combinado com `--json`
- `--user-agent` sobrescreve o cabeçalho User-Agent HTTP
- `--verbose` ativa logging nível INFO no stderr (video_id_extracted, started, cache_hit, completed)
- `--quiet` suprime todo output de log não erro no stderr
- `--verbose` e `--quiet` são mutuamente exclusivos
- `--log-level` aceita `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` aceita `text` ou `json`
- `--color` aceita `auto`, `always`, `never`
- NUNCA passe providers removidos (`youtube-direct`, `provider-a`, `provider-b`, `provider-headless`)
- NUNCA passe flags removidas (`--asr`, `--no-fallback`, `--headless`)
- NUNCA combine `--batch` com URL posicional — exit 64
- NUNCA combine `--quiet` com `--verbose` — exit 64
- NUNCA passe `--timeout 0` ou `--cache-ttl 0` — exit 64
- NUNCA hardcode hostnames em scripts


## Envelope JSON e NDJSON
- VALIDE o campo `error` ANTES de confiar em qualquer outro campo
- Campos do envelope de sucesso — `provider` (provider-noteey ou cache), `video_id`, `language` (locale solicitado), `language_detected` (SEMPRE false), `format`, `content` (texto limpo NFC), `byte_size` (tamanho exato em bytes do content), `duration_ms` (tempo wall-clock em ms), `source_url`
- `language_detected` é SEMPRE false porque noteey.com NÃO tem seletor de idioma
- `byte_size` reflete o tamanho EXATO em bytes do campo `content` após parsing e normalização NFC
- Campos do envelope de erro — `error` (sempre true), `code` (exit code BSD sysexits), `message`
- TODOS os erros emitem JSON estruturado no stdout quando `--json` está ativo, INCLUINDO erros de validação pre-fetch
- `--batch --json` emite NDJSON (um objeto JSON por linha, terminado por newline) — NUNCA JSON concatenado
- Cada objeto NDJSON é autocontido e parseável independentemente por `jaq`
- LEIA `retry_after_seconds` quando presente em envelopes de erro
- NUNCA leia `.body` — o campo se chama `.content`
- NUNCA faça parse do stdout linha a linha como texto cru quando `--json` está ativo
- NUNCA assuma que `content` é sempre não vazio
- NUNCA assuma que saída batch é um array JSON — é NDJSON


## Códigos de Saída
- `0` — sucesso
- `2` — rejeição do parser clap (valor de flag inválido)
- `64` EX_USAGE — combinações de flag inválidas, entrada inválida, `--format srt` com provider-noteey
- `65` EX_DATAERR — URL do YouTube malformada ou não reconhecida
- `66` EX_NOINPUT — vídeo não tem legenda correspondente
- `69` EX_UNAVAILABLE — provider indisponível, rate-limited, Chromium ausente, ou captcha
- `70` EX_SOFTWARE — falha interna, timeout, HTTP, I/O ou erro de parse
- `78` EX_CONFIG — arquivo de configuração malformado ou ilegível
- `130` — SIGINT ou SIGTERM interrupção do usuário
- NUNCA mascare o exit code com `|| true`
- NUNCA trate `69` como falha permanente — Chromium ausente e captcha são recuperáveis


## Provider e Chromium
- O provider EXCLUSIVO é `provider-noteey` controlando noteey.com via Chromium headless
- O provider navega para noteey.com, preenche o input de URL, clica "Get Subtitle" e faz poll do painel de transcrição por até 30 segundos
- noteey.com retorna UMA transcrição no idioma ORIGINAL do vídeo — NÃO há seletor de idioma
- Patches stealth anti-fingerprint são injetados via CDP antes da navegação
- Ordem de resolução do Chromium — (1) variável `$CHROME`, (2) auto-download do BrowserFetcher da revisão `r1585606` em `~/.cache/youtube-legend-cli/browser/`, (3) caminhos de sistema como fallback
- PREFIRA a revisão do BrowserFetcher sobre browser de sistema para evitar incompatibilidade de protocolo CDP
- DEFINA `$CHROME` para fixar um binário compatível
- Perfil do chrome em `~/.cache/youtube-legend-cli/chrome-profile/`
- NUNCA espere que `--format srt` funcione
- NUNCA espere que noteey retorne legendas em idioma específico
- NUNCA espere que captcha se resolva por retry — exit 69


## Cache e Retry
- TTL padrão de 24 horas em disco em `~/.cache/youtube-legend-cli/`
- Sobrescreva a raiz do cache com `$YT_LEGEND_CACHE_DIR`
- USE `--no-cache` para fetches frescos
- USE `--cache-ttl` para TTL customizado em horas
- Invalide uma entrada removendo seu diretório
- LEIA `retry_after_seconds` de envelopes de erro em HTTP 429
- Fallback interno 60s, teto 300s
- NUNCA delete o diretório inteiro de cache em produção
- NUNCA rode loops de retry sem backoff
- NUNCA defina `--timeout` abaixo de 5 segundos


## Variáveis de Ambiente
- `CHROME` — fixa o executável Chromium, pula BrowserFetcher
- `YT_LEGEND_NO_NETWORK` — desabilita rede, retorna exit 69
- `YT_LOG_LEVEL` — vence `--log-level`
- `YT_LOG_FORMAT` — vence `--log-format`
- `YT_LEGEND_CACHE_DIR` — sobrescreve diretório de cache XDG
- `NO_COLOR` e `CLICOLOR_FORCE` são honrados quando `--color` é `auto`
- USE a família `YT_*` para qualquer override
- NUNCA defina `RUST_LOG` quando `YT_*` se aplica


## Arquivo de Config (TOML)
- `--config <PATH>` carrega tabela TOML com chaves espelhando flags longas sem `--`
- Precedência — flag CLI > valor do config > default embutido
- Chaves suportadas — `url`, `lang`, `format`, `timeout`, `cache_ttl`, `user_agent`, `provider`
- Chaves booleanas — `verbose`, `quiet`, `json`, `batch`, `no_cache`, `dry_run`, `no_progress`, `yes`
- Opcionais — `log_level`, `log_format`, `color`
- Um config mínimo contém `lang = "pt"`, `format = "txt"`, `cache_ttl = 24`
- NUNCA adicione chave desconhecida — exit 78
- NUNCA escreva TOML malformado — exit 78


## Tratamento de Erros
- RAMIFIQUE pelo exit code para determinar a categoria de erro
- `BrowserNotFound` (exit 69) — instale um browser ou defina `$CHROME`
- `CaptchaChallenge` (exit 69) — requer interação humana, NÃO resolve por retry
- `NoSubtitle` (exit 66) — não existe transcrição para o idioma solicitado
- `RateLimited` (exit 69) — leia `retry_after_seconds` e aguarde
- TODOS os erros emitem JSON estruturado quando `--json` está ativo
- NUNCA entre em panic em exit não zero
- NUNCA transforme erros em string para casar por substring


## Fórmulas Prontas
- FÓRMULAS POR FLAG (todas as 18 flags)
- BAIXAR vídeo único — `youtube-legend-cli "https://youtu.be/VIDEO" > legenda.txt`
- EXTRAIR conteúdo via JSON — `youtube-legend-cli --json "https://youtu.be/VIDEO" 2>/dev/null | jaq -r '.content'`
- SELECIONAR idioma — `youtube-legend-cli --lang pt-BR "https://youtu.be/VIDEO" > legenda.txt`
- SELECIONAR idioma com variante locale — `youtube-legend-cli --lang pt_BR.UTF-8 "https://youtu.be/VIDEO"`
- FORMATO txt explícito — `youtube-legend-cli --format txt "https://youtu.be/VIDEO" > limpo.txt`
- DEFINIR timeout customizado — `youtube-legend-cli --timeout 60 "https://youtu.be/VIDEO"`
- CARREGAR arquivo de config — `youtube-legend-cli --config ./yt-legend.toml "https://youtu.be/VIDEO"`
- SOBRESCREVER TTL do cache — `youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"`
- FORÇAR leitura fresca — `youtube-legend-cli --no-cache "https://youtu.be/VIDEO" > fresco.txt`
- SUPRIMIR barras de progresso — `youtube-legend-cli --no-progress "https://youtu.be/VIDEO" > legenda.txt 2>/dev/null`
- DRY RUN somente cache — `youtube-legend-cli --dry-run "https://youtu.be/VIDEO"`
- BATCH não interativo — `youtube-legend-cli --yes --batch < urls.txt > saida.txt`
- BATCH com NDJSON — `cat urls.txt | youtube-legend-cli --batch --json 2>/dev/null | jaq -r 'select(.error == null) | .content'`
- USER-AGENT customizado — `youtube-legend-cli --user-agent "MeuBot/1.0" "https://youtu.be/VIDEO"`
- DEBUG verboso — `youtube-legend-cli --verbose --log-level debug "https://youtu.be/VIDEO" > sub.txt 2> trace.log`
- MODO silencioso — `youtube-legend-cli --quiet "https://youtu.be/VIDEO" > legenda.txt`
- FORMATO de log JSON — `YT_LOG_FORMAT=json youtube-legend-cli --log-format json --json "https://youtu.be/VIDEO" 2> logs.jsonl`
- SEM COR em CI — `youtube-legend-cli --color never --json "https://youtu.be/VIDEO"`
- FIXAR provider — `youtube-legend-cli --provider provider-noteey "https://youtu.be/VIDEO"`
- EXTRAÇÃO DE CAMPOS DO ENVELOPE
- EXTRAIR video_id — `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.video_id'`
- VERIFICAR language_detected — `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.language_detected'`
- LER byte_size — `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.byte_size'`
- LER duration_ms — `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.duration_ms'`
- LER source_url — `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.source_url'`
- LER provider — `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.provider'`
- EXTRAIR código de erro — `youtube-legend-cli --json "URL_INVALIDA" 2>/dev/null | jaq '.code'`
- EXTRAIR mensagem de erro — `youtube-legend-cli --json "URL_INVALIDA" 2>/dev/null | jaq -r '.message'`
- PADRÕES COMBINADOS
- PARSEAR envelope com segurança — `out=$(youtube-legend-cli --json "$url" 2>/dev/null) && echo "$out" | jaq -e '.error == null' >/dev/null && echo "$out" | jaq -r '.content' || echo "$out" | jaq '{code: .code, message: .message}'`
- ROTEAR por exit code — `youtube-legend-cli "$url" || case $? in 66) echo "sem legendas";; 69) echo "provider indisponivel";; *) echo "falha";; esac`
- CI fresco sem progresso JSON — `youtube-legend-cli --json --no-cache --no-progress --provider provider-noteey "$url" > out.json 2> trace.log`
- BATCH silencioso NDJSON — `cat urls.txt | youtube-legend-cli --batch --json --quiet --no-progress 2>/dev/null | jaq -c 'select(.error == null) | {id: .video_id, bytes: .byte_size}'`
- MODO offline — `YT_LEGEND_NO_NETWORK=1 youtube-legend-cli "$url"` (retorna exit 69)
- FIXAR Chromium — `CHROME=/usr/bin/chromium youtube-legend-cli "$url"`
- PIPELINE de auditoria fresca — `youtube-legend-cli --no-cache --json "$url" 2>/dev/null | jaq -r '.content' > auditoria-fresca.txt`
