# gaps


## GAP-AUD-2026-060: JSON error envelope ausente para erros pre-fetch
- SEVERIDADE: ALTA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/commands/mod.rs:60-67`, `src/commands/extract.rs:33-48`
- CAUSA: `cli.validate()` usava `?` que propagava o erro para `main.rs` sem passar por `output_error()`
- EFEITO: quando `--json` estava ativo e ocorria erro de validacao, stdout ficava VAZIO
- CORRECAO APLICADA: interceptar erros com `if let Err(e)` e rotear para `output_error()` em `commands::run()` e `extract::run()`
- VALIDACAO: `youtube-legend-cli --json "URL_INVALIDA"` agora emite `{"error":true,"code":65,"message":"..."}` com exit 65


## GAP-AUD-2026-061: campo language no envelope reflete idioma SOLICITADO nao REAL
- SEVERIDADE: MEDIA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/commands/mod.rs:20-22,200`
- CAUSA RAIZ: noteey.com retorna UMA unica transcrição por pagina, sem selecao por idioma. O provider sempre retorna a legenda no idioma original do video
- EFEITO: `--lang en` em video PT retornava `language: "en"` sem sinal de que o idioma nao foi detectado
- CORRECAO APLICADA: campo `language_detected: false` adicionado ao envelope JSON, documentacao atualizada em `docs/AGENTS.md` e `docs/AGENTS.pt-BR.md`
- VALIDACAO: envelope agora inclui `"language_detected":false` em toda resposta


## GAP-AUD-2026-062: marcadores >> de speaker change nao limpos pelo parser
- SEVERIDADE: BAIXA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/parse/mod.rs:176-179`
- CAUSA: `noteey_to_text()` filtrava `[Music]` e `(Applause)` mas ignorava `>>`
- EFEITO: 164 linhas com `>>` residual em video de entrevista
- CORRECAO APLICADA: `strip_prefix(">>")` apos o strip de timestamp, antes da verificacao de marker-only
- VALIDACAO: video gZPoUOkwFKo agora retorna 0 linhas com `>>`, 3 testes unitarios adicionados


## GAP-AUD-2026-063: documentacao AGENTS.md diz body mas envelope JSON usa content
- SEVERIDADE: MEDIA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `docs/AGENTS.md:58,115-125`, `docs/AGENTS.pt-BR.md:57,115-125`
- CAUSA: campo foi renomeado de `body` para `content` em GAP-AUD-2026-050 mas a documentacao nao foi atualizada
- EFEITO: consumidores programaticos que seguem a documentacao tentam ler `.body` e recebem `null`
- CORRECAO APLICADA: `.body` atualizado para `.content` em exemplos jq e schema JSON de ambos AGENTS.md e AGENTS.pt-BR.md; campo `language_detected` adicionado ao schema
- NOTA: CLAUDE.md nao foi alterado (PROIBIDO); a divergencia no skill nao pode ser corrigida por este projeto


## GAP-AUD-2026-065: byte_size no envelope mede HTML bruto em vez do content limpo
- SEVERIDADE: MEDIA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/commands/extract.rs:66,145`
- CAUSA: `byte_size` era calculado como `content.len()` (body bruto do provider) e `bytes.len()` (cache bruto), mas o campo `content` no JSON passa por `noteey_to_text()` + `normalize_nfc()`
- EFEITO: Video 1 reportava `byte_size: 23572` mas content real tinha 19568 bytes (diferenca de 4004 bytes)
- CORRECAO APLICADA: recalcular `byte_size` a partir de `converted.len()` (texto limpo pos-parsing) em ambos os caminhos (cache hit e provider fetch)
- VALIDACAO: `byte_size` agora bate com o tamanho real do `content` no JSON (19568 bytes)


## GAP-AUD-2026-066: flag --verbose era dead flag sem efeito no logging
- SEVERIDADE: MEDIA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/cli.rs:359-371`
- CAUSA: `--verbose` era definida no clap e aceita pelo parser, mas `effective_log_level()` nao a consultava; `apply_overrides()` setava `YT_LOG_LEVEL=warn` antes de `init_tracing()`, sobrescrevendo qualquer logica de verbose
- EFEITO: `--verbose` nao produzia nenhum output adicional no stderr
- CORRECAO APLICADA: `effective_log_level()` agora retorna `LogLevelArg::Info` quando `verbose=true` e `log_level` esta no default (`Warn`), fazendo o env var propagado conter o nivel correto
- VALIDACAO: `--verbose` agora emite 4 linhas INFO no stderr (video_id_extracted, started, cache_hit, completed)


## GAP-AUD-2026-067: stderr emite kill signal failed no cleanup do Chromium
- SEVERIDADE: BAIXA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/provider/provider_noteey.rs:234-240`
- CAUSA: `handler_task.abort()` dropava o Handler que tentava kill no processo Chromium ja encerrado; `Browser::Drop` tambem tentava kill duplicado
- EFEITO: stderr mostrava `kill: sending signal to PID failed: Processo inexistente` em cada execucao
- CORRECAO APLICADA: `std::mem::forget(browser)` previne Browser::Drop; substituir `abort()` por timeout wait de 3s permite que o Handler observe o fechamento do WebSocket e termine naturalmente antes do Drop
- VALIDACAO: stderr limpo em ambos os videos de teste (zero linhas kill)


## GAP-AUD-2026-068: --format srt limitacao do provider-noteey nao documentada no help text
- SEVERIDADE: INFO
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/cli.rs:187-193`
- CAUSA: noteey.com retorna transcript sem SRT framing; `--format srt` retorna exit 64 com mensagem clara mas o help text do clap nao menciona a limitacao
- EFEITO: usuario descobre a limitacao apenas no erro de runtime em vez de no `--help`
- CORRECAO APLICADA: help text atualizado para `txt (default) or srt (unavailable with provider-noteey)`
- VALIDACAO: `--help` agora exibe a limitacao diretamente


## GAP-AUD-2026-064: erro duplicado no stderr para URL invalida
- SEVERIDADE: BAIXA
- STATUS: CORRIGIDO (2026-06-23) como efeito colateral do GAP-060
- ARQUIVO: `src/commands/mod.rs`
- CAUSA: erros propagados via `?` acionavam `#[instrument(err)]` E o `Termination::report` em `main.rs`
- EFEITO: URL invalida produzia 2 linhas identicas no stderr
- CORRECAO APLICADA: com o fix do GAP-060, erros de validacao retornam `Ok(ExitCode)` sem acionar `instrument(err)`
- VALIDACAO: stderr agora emite 1 linha em vez de 2 para erros de validacao


## GAP-AUD-2026-069: batch --json concatena envelopes sem newline (NDJSON quebrado)
- SEVERIDADE: ALTA
- STATUS: CORRIGIDO (2026-06-23)
- ARQUIVO: `src/commands/mod.rs:204-206,231-233,258-260`
- CAUSA: `output_success`, `output_error` e `output_dry_run` escreviam JSON via `write_subtitle_to_stdout` sem `\n` final
- EFEITO: `--batch --json` produzia `}{` concatenado entre envelopes, quebrando parsers NDJSON como `jaq`
- REPRODUZIR: `printf 'URL1\nURL2' | youtube-legend-cli --batch --json` e verificar `wc -l` retornava 0
- CORRECAO APLICADA: `json.push('\n')` antes de cada write em `output_success`, `output_error` e `output_dry_run`
- VALIDACAO: batch retorna 2 linhas NDJSON validas, zero `}{` concatenados, `jaq` parseia ambos envelopes
