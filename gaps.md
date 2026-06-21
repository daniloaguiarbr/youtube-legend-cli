====



## GAP-AUD-2026-041 — noteey.com pode bloquear via fingerprint anti-bot (Cloudflare)

### Problema
- O site noteey.com está protegido por Cloudflare Turnstile.
- O TLS fingerprint do headless Chromium via chromiumoxide tem assinaturas conhecidas.
- Sites com Windsor.io e Cloudflare Bot Manager agregam sinais do browser.
- O fingerprint passivo r.wdfl.co/rw.js lê navigator.webdriver antes da interação.
- Referência canônica: chromiumoxide-0.9.1/src/page.rs:1253-1279 documenta que evaluate_on_new_document é o método oficial para Hiding automation detection properties — exatamente o caso de uso deste gap.

### Consequências
- CLI retorna `ProviderUnavailable` quando noteey detecta bot.
- Latência sobe para ~30s porque o provider tenta até o timeout esgotar.
- Operador não tem visibilidade do bloqueio sem inspecionar logs estruturados.
- Em datacenter, taxa de bloqueio pode chegar a 100% dos requests.

### Causa Raiz
- O TLS handshake do Chromium puro tem cipher suites reveladoras.
- O header `User-Agent` contém `HeadlessChrome` em builds antigas do chromiumoxide.
- O `navigator.webdriver === true` é exposto por padrão no chromiumoxide 0.9.1.
- O `navigator.plugins` array está vazio em headless mode.
- O `navigator.languages` tem apenas `["en-US"]` em headless mode.
- O WebGL vendor expõe `Google Inc. (SwiftShader)` em vez de vendor real.

### Solução
- Adicionar 5 patches via CDP `Page.addScriptToEvaluateOnNewDocument` em `src/provider/stealth.rs`.
- Patch 1: `navigator.webdriver` retorna `undefined`.
- Patch 2: `navigator.plugins` populado com 3 entradas PDF Plugin.
- Patch 3: `navigator.languages` com `["pt-BR", "en-US", "en"]`.
- Patch 4: WebGL vendor override para `Intel Inc.`.
- Patch 5: `window.chrome.runtime` mockado.
- Adicionar flag `--disable-blink-features=AutomationControlled` em `BrowserConfig`.
- Usar `BrowserFetcher` (já habilitado via `features = ["fetcher"]`) para baixar Chromium real.

### Benefícios da Solução
- Fingerprint passa em `https://fingerprintjs.github.io/fingerprintjs/`.
- Vendor WebGL reporta `Intel Inc.` em `https://browserleaks.com/webgl`.
- Score do Cloudflare Bot Manager cai abaixo do threshold de bloqueio.
- Latência volta ao normal de ~5s em vez de 30s com timeout.

### Causa × Efeito
| Sinal fingerprint | Causa | Consequência |
|-------------------|-------|--------------|
| `navigator.webdriver === true` | Chromiumoxide padrão expõe | Bot detectado |
| `HeadlessChrome` no UA | UA default do chromium | Cloudflare flagra |
| `plugins` array vazio | Headless mode não carrega plugins | FingerprintJS score baixo |
| `languages: ["en-US"]` | Locale padrão do chromiumoxide | FingerprintJS detect |
| WebGL vendor SwiftShader | Software rasterizer em headless | Browserleaks detect |

### Como Solucionar
- Criar `src/provider/stealth.rs` com constante `STEALTH_INIT_JS` e função `apply_stealth(&Page)`.
- Modificar `src/provider/provider_noteey.rs::drive_page` para chamar `apply_stealth` antes do `page.goto`.
- Modificar `src/provider/provider_headless.rs::drive_page` com o mesmo patch.
- Adicionar flag `--disable-blink-features=AutomationControlled` no `BrowserConfig::builder()`.
- Adicionar teste `tests/integration/stealth_wiremock.rs` validando cada patch isoladamente.
- Validar manualmente contra `fingerprintjs.github.io/fingerprintjs/`.

### Por Que Esta É a Causa Raiz Unificada
- GAP-AUD-2026-044 é o problema mais amplo — mesmo fingerprint afeta downsub e noteey.
- Fechar GAP-041 sozinho via mitigação parcial não basta.
- BrowserFetcher mitiga TLS fingerprint mas não cobre `navigator.webdriver`.

### Verificação
- Teste contra `fingerprintjs.github.io` reporta `navigator.webdriver: false`.
- Teste contra `browserleaks.com/webgl` reporta vendor `Intel Inc.`.
- Teste contra `amiunique.org` reporta fingerprint comum.
- Latência cai de 30s para ~5s com provider-noteey bem-sucedido.
- Provider-unavailable rate cai abaixo de 5% em datacenter.

### Próximo Passo
- Roadmap v0.3.3 — implementar GAP-044 (stealth patches completos).
- Este gap será fechado junto com GAP-044 em release única.


## GAP-AUD-2026-042 — API interna do noteey pode mudar endpoints sem aviso

### Problema
- A extração do transcript do noteey depende da estrutura DOM atual do site.
- O site usa Vue.js que pode mudar seletores e estrutura sem aviso.
- O caminho atual em `provider_noteey.rs::EXTRACT_JS` depende de:
  - Atributos `[data-transcript]` ou `[class*="transcript"]`.
  - Presença de pelo menos 3 timestamps `MM:SS` no container.

### Consequências
- CI pode quebrar silenciosamente quando noteey publica update.
- Latência sobe para ~30s porque o polling esgota sem encontrar transcripts.
- Operador vê `noteey_extract_diagnostic` no log mas precisa analisar manualmente.
- Em update major do noteey, gap pode virar blocker de funcionalidade.

### Causa Raiz
- Sites SPA alteram DOM arbitrariamente em cada release.
- Seletores CSS estáveis raramente são承诺ados por vendors.
- API interna do noteey (se existir) é desconhecida — ver GAP-043.
- Fica acoplado ao estado atual do front-end sem contrato formal.

### Solução
- Implementar fallback em camadas: tentar seletor estável primeiro.
- Tentar `[data-transcript]` → fallback `[class*="transcript"]` → fallback heurística de timestamps.
- Se todos falharem, retornar `ProviderUnavailable` para que chain tente outros providers.
- Adicionar metric `noteey_extraction_strategy_used` em tracing para diagnóstico.
- Considerar GAP-043 (provider-noteey-http) como caminho alternativo quando DOM mudar.

### Benefícios da Solução
- 3 estratégias de fallback dão resiliência contra mudanças parciais.
- Operator tem visibilidade via tracing qual estratégia foi usada.
- Se DOM mudar completamente, chain ainda tenta outros providers antes de falhar.

### Causa × Efeito
| Sintoma | Causa | Consequência |
|---------|-------|--------------|
| `noteey_extract_diagnostic` no log | Polling esgotou sem encontrar transcripts | Latência 30s |
| `noteey returned empty body` | Seletor mudou | ProviderUnavailable |
| Transcript sem header do site | `findTranscriptRegion` retornou container errado | Header vaza |
| `body_dump` mostra hero da página | DOM reestruturado | Chain degrada |

### Como Solucionar
- Editar `src/provider/provider_noteey.rs::EXTRACT_JS` para incluir 3 níveis de fallback.
- Adicionar `tracing::debug!(target: "events", strategy_used = "...")` em `drive_page`.
- Documentar quais seletores foram tentados em ordem no doc-comment.

### Verificação
- Mock do noteey com HTML antigo deve ativar fallback para heurística.
- Mock do noteey com HTML novo deve cair para heurística com sucesso.
- Operador consegue identificar qual estratégia foi usada via `RUST_LOG=debug`.

### Próximo Passo
- Mitigado parcialmente pelo `findTranscriptRegion` em v0.3.2.
- Combinar com GAP-043 quando provider-noteey-http ficar viável.
- v0.3.3 ou v0.4.0 — depende da estabilidade observada do noteey.


## GAP-AUD-2026-043 — provider-noteey poderia usar API direta HTTP em vez de browser

### Problema
- O provider-noteey atual usa chromiumoxide para dirigir o SPA Vue do noteey.
- Cada invocação spawna um browser headless (latência 5-30s).
- O noteey provavelmente expõe uma API HTTP interna que retorna o JSON do transcript.
- O caminho via browser depende do GAP-041 (fingerprint anti-bot).
- Referência canônica: chromiumoxide-0.9.1/src/page.rs:1253-1279 confirma que o caminho browser-based é o único oficial. A doc cita explicitamente Hiding automation detection properties como caso de uso prioritário de evaluate_on_new_document. O fato de a upstream library recomendar patches via DOM em vez de expor API HTTP alternativa valida a hipótese de que não existe caminho HTTP público exposto.

### Consequências
- Latência 5-30s por vídeo em vez de 200-500ms via API direta.
- CPU e RAM consumidos por chromiumoxide desnecessariamente.
- GAP-041 e GAP-044 amplificam o problema — API pode estar acessível mesmo quando browser é bloqueado.
- Em ambientes com fingerprinting agressivo, browser é bloqueado mas API pode passar.

### Causa Raiz
- Decisão de design original priorizou robustez via DOM extraction (GAP-042).
- Mapeamento youtube_id → noteey_share_id é desconhecido.
- Pesquisa de mappings exige análise manual do JS do noteey.
- MCP Firefox devtools não está disponível neste ambiente (ver feedback-mcp-firefox-unavailable).

### Solução
- Mapear 5 youtube_ids conhecidos para share_ids via análise manual do JS.
- Implementar `provider_noteey_http.rs` como alternativa ao `provider_noteey` browser-based.
- Chain ordem: `provider-noteey-http` primeiro, fallback `provider-noteey` browser.
- HTTP API call via reqwest já disponível no projeto.
- Validar com 10 vídeos diferentes antes de promover.

### Benefícios da Solução
- Latência cai de 30s para ~500ms em 80% dos casos.
- Reduz dependência de chromiumoxide e do fingerprint anti-bot.
- Provider-noteey HTTP pode coexistir com browser version como fallback.
- Memória e CPU liberados para outros processos.

### Causa × Efeito
| Sintoma | Causa | Consequência |
|---------|-------|--------------|
| Latência 30s em noteey | Browser spawn é lento | CPU/RAM consumido |
| Provider-unavailable em datacenter | GAP-044 fingerprint | API bloqueada mas HTTP livre |
| Single point of failure | Browser-only | Se GAP-044 não fechar, noteey inteiro morre |
| Reuso do reqwest | HTTP é trivial | Já temos `provider_a.rs` HTTP-based |

### Como Solucionar
- Analisar `https://www.noteey.com/youtube-subtitle-downloader` no DevTools.
- Capturar request de API quando usuário submete URL.
- Mapear 5 youtube_ids → share_ids empiricamente.
- Criar `src/provider/provider_noteey_http.rs` que faz POST/GET direto.
- Adicionar feature flag `provider-noteey-http` em `Cargo.toml`.
- Chain ordem em `commands/mod.rs::build_provider_chain`:
  - `provider-noteey-http` quando `cfg!(feature = "provider-noteey-http")`.
  - `provider-noteey` browser como fallback.

### Verificação
- 10 vídeos diferentes retornam em <500ms via HTTP.
- Quando HTTP falha, browser fallback entra em ação sem overhead inicial.
- `YT_LEGEND_NO_NETWORK=1` ainda respeita contrato (ProviderUnavailable).

### Próximo Passo
- **Deferred** — pesquisa de mappings bloqueada em ambiente sem MCP Firefox devtools.
- v0.3.3+ — depende do desbloqueio do MCP Firefox devtools.
- Caso o fingerprint (GAP-044) seja fechado com sucesso, GAP-043 perde prioridade.
- Caso fingerprint persista bloqueado, GAP-043 vira caminho crítico.


## GAP-AUD-2026-044 — chromiumoxide headless trivialmente detectável via fingerprint anti-bot (PRIORITÁRIO)
### Problema
- O chromiumoxide 0.9.1 não injeta patches anti-detecção por padrão.
- O TLS handshake do Chromium revela assinatura chromiumoxide para Cloudflare.
- O navigator.webdriver === true é exposto por padrão em todo chromiumoxide::Page.
- O window.chrome.runtime está ausente por padrão.
- O navigator.plugins array está vazio.
- O WebGL vendor expõe Google Inc. (SwiftShader).
- Referência canônica: chromiumoxide-0.9.1/src/page.rs:1253-1279 (evaluate_on_new_document) é o método oficial documentado para Hiding automation detection properties. Exemplo oficial:
  ```rust
  page.evaluate_on_new_document(r#"
      Object.defineProperty(Object.getPrototypeOf(navigator), webdriver, {
          get: () => false
      });
  "#).await?;
  ```
- Alias equivalente: page.add_init_script é sugar para evaluate_on_new_document.
- O WebGL vendor expõe `Google Inc. (SwiftShader)`.

### Consequências
- Cloudflare Bot Manager atribui score alto de risco a todo request.
- Sites com Windsor.io (`r.wdfl.co/rw.js`) bloqueiam antes mesmo do challenge.
- provider-noteey e provider-headless ambos bloqueados em datacenter.
- Provider-unavailable rate pode chegar a 100% em ambientes institucionais.
- Latência sobe para 30s porque o polling esgota.

### Causa Raiz
- chromiumoxide 0.9.1 foi desenhado para testing/debugging, não para evasão.
- Não há método built-in para mascarar `navigator.webdriver`.
- BrowserFetcher mitiga TLS fingerprint mas não cobre os 5 sinais principais.
- Decisão de design da upstream library priorizou transparência sobre evasão.

### Solução
- Criar `src/provider/stealth.rs` compartilhado entre providers headless.
- Injetar 5 patches via CDP `Page.addScriptToEvaluateOnNewDocument` antes de qualquer navegação.
- Aplicar imediatamente após `browser.new_page("about:blank")` e antes de `page.goto`.
- Adicionar flag `--disable-blink-features=AutomationControlled` em `BrowserConfig`.
- Documentar ordem de execução no doc-comment do módulo.

### Benefícios da Solução
- Provider-noteey e provider-headless funcionam em datacenter sem proxy.
- Latência cai de 30s para ~5s em chamadas bem-sucedidas.
- Reduz dependência de proxies externos para contornar bloqueios.
- Cria módulo compartilhado que ambos providers podem reusar.

### Causa × Efeito
| Sinal exposto | Mecanismo de detecção | Impacto |
|---------------|----------------------|---------|
| `navigator.webdriver = true` | FingerprintJS, Cloudflare | Bloqueio imediato |
| `HeadlessChrome` no UA | Cloudflare regex match | Score alto |
| `plugins = []` | FingerprintJS | Fingerprint único |
| WebGL SwiftShader | Browserleaks, Amiunique | Fingerprint detectável |
| `chrome.runtime` ausente | Sites que checam API | Score alto |

### Como Solucionar
- Criar `src/provider/stealth.rs`:
  - Constante `STEALTH_INIT_JS` com os 5 patches inline.
  - Função `apply_stealth(page: &Page) -> AppResult<ScriptIdentifier>`.
  - Documentação explicando que precisa rodar em `about:blank` antes de `goto`.
- Modificar `src/provider/provider_headless.rs::drive_page`:
  - Adicionar `arg("--disable-blink-features=AutomationControlled")` no `BrowserConfig::builder()`.
  - Adicionar `arg("--disable-features=AutomationControlled")`.
  - Chamar `apply_stealth(&page)` antes do `page.goto`.
- Modificar `src/provider/provider_noteey.rs::drive_page`:
  - Mesmas flags e chamada de `apply_stealth`.
- Adicionar `tests/integration/stealth_wiremock.rs`:
  - `stealth_init_js_contains_webdriver_patch`.
  - `stealth_init_js_contains_plugins_patch`.
  - `stealth_init_js_contains_languages_patch`.
  - `stealth_init_js_contains_webgl_patch`.
  - `stealth_init_js_contains_chrome_runtime_patch`.

### Verificação
- Teste contra `https://fingerprintjs.github.io/fingerprintjs/` reporta `navigator.webdriver: false`.
- Teste contra `https://browserleaks.com/webgl` reporta vendor `Intel Inc.`.
- Teste contra `https://amiunique.org` reporta fingerprint comum.
- Latência total cai de 30s para ~5s em chamadas bem-sucedidas.
- Provider-unavailable rate cai abaixo de 5% em datacenter.

### Por Que Esta É a Causa Raiz Unificada
- GAP-AUD-2026-041 (Cloudflare noteey) é subset deste gap.
- GAP-AUD-2026-042 (API noteey instável) é independente mas mitigado por DOM extraction.
- A correção do GAP-044 destrava o fallback automático que o usuário pediu na v0.3.2.
- Sem GAP-044 fechado, o provider_noteey e o provider_headless continuam bloqueados.
- A release v0.3.3 deve resolver GAP-044 antes de qualquer outro gap de provider headless.

### Próximo Passo
- v0.3.3 — prioridade alta, bloqueia o objetivo do usuário.
- Implementar `src/provider/stealth.rs` como módulo compartilhado.
- Aplicar em ambos providers em paralelo.
- Validar com suite de testes contra sites de fingerprint detection.
- Documentar em `CHANGELOG.md` e `docs/AGENTS.pt-BR.md`.


====


## GAP-AUD-2026-045 — terminal CDP error em submit/extract retorna Internal em vez de ProviderUnavailable

### Problema
- `provider_noteey.rs` e `provider_headless.rs` chamam `page.evaluate(...)` que pode retornar `chromiumoxide::error::CdpError::ChannelClosed` quando o target CDP é fechado por navegação ou kill upstream.
- O código antigo (pré-fix) tratava isso como `AppError::Internal`, o que retornava exit 70 `EX_SOFTWARE` e bloqueava o chain.
- Para `provider-headless`, isso significava que qualquer navigation do downsub.com durante o extract matava a sessão inteira, sem fallback para noteey.

### Consequências
- Session inteira perdida quando o target CDP fecha (Cloudflare challenge navigation, browser kill upstream, target inspected and closed).
- Chain termina com exit 70 em vez de tentar noteey como fallback.
- Operador vê `Internal("submit evaluate failed: ...")` em logs sem indicação de que o problema é transitório e degradável.

### Causa Raiz
- `is_terminal_cdp_error(&CdpError)` classification existed em `provider_headless.rs:37` mas não era usada em todos os pontos onde o erro poderia aparecer.
- A premissa implícita era "qualquer erro de evaluate é erro nosso", o que é incorreto — erros terminais são upstream-induced.

### Solução
- `provider_noteey.rs:321-334`: `submit_evaluate` checa `is_terminal_cdp_error(&e)` antes de retornar error. Se terminal, retorna `AppError::ProviderUnavailable` (degraded).
- `provider_noteey.rs:371-380`: `extract_evaluate` mesmo tratamento.
- `provider_headless.rs:588-602`: `download_evaluate` mesmo tratamento.
- O chain wrap em `provider/mod.rs:412-434` (já implementado no GAP-049) classifica `ProviderUnavailable` como `degraded = true` e continua para o próximo provider.

### Benefícios da Solução
- Chain continua automaticamente quando target CDP morre.
- Operador vê `submit_evaluate_terminal; aborting` em tracing estruturado com severidade warn, distinguível de erros genuínos.
- noteey vira fallback natural para downsub quando downsub mata o target via Cloudflare challenge.

### Causa × Efeito
| Sintoma | Causa | Efeito |
|---------|-------|--------|
| Exit 70 após navegação Cloudflare | Internal error propagado sem classification | Chain termina prematuramente |
| Noteey nunca tenta após downsub navegar | Chain termina em downsub | Fallback automático quebrado |
| `submit_evaluate_failed` no log sem contexto | Tratamento uniforme de erros | Operador não distingue terminal de transient |

### Como Solucionar
- Adicionar `is_terminal_cdp_error(&e)` check em todos os sites onde `page.evaluate(...).await` retorna `Err(e)`.
- Mapear para `AppError::ProviderUnavailable` quando terminal.
- Emitir tracing warn com mensagem `*_evaluate_terminal; aborting`.

### Próximo Passo
- ✅ Corrigido nesta sessão
- Cobertura completa em ambos providers headless
- Operadores com Cloudflare challenges agora fazem fallback automático para noteey


## GAP-AUD-2026-046 — noteey_extract_diagnostic dump primeiros 500 chars quando polling esgota

### Problema
- `provider_noteey.rs::drive_page` faz polling do transcript pane por até 30 segundos (`NOTEEY_POLL_LIMIT = 30`).
- Quando o polling esgota sem encontrar `≥3` timestamps, o código antigo retornava `AppError::ProviderUnavailable` sem indicar o que aconteceu no body.
- Operador não sabia distinguir: captcha interstitial, página vazia (degradação upstream), ou DOM lento (render parcial).

### Consequências
- Diagnóstico cego: chain tenta provider-noteey, esgota, cai em `ProviderUnavailable`. Operador precisa executar manualmente no browser para entender.
- Latência 30s desperdiçada sem informação útil.
- CI scripts não conseguem distinguir falha real de flaky.

### Causa Raiz
- O `EXTRACT_JS` retorna `{err, polled, last_body_len}` mas NÃO o conteúdo do body.
- Rust side não tinha como reconstruir o que o JS viu sem fazer um segundo fetch ou re-avaliar JS.

### Solução
- `provider_noteey.rs:110`: `EXTRACT_JS` agora retorna `body: lastBody` em vez de apenas `last_body_len`.
- `provider_noteey.rs:394-410`: Rust side lê `body` do JSON, pega primeiros 500 chars via `body_dump.chars().take(500).collect()` e emite via `tracing::warn!(target: "events", ..., first_500 = %first_500, "noteey_extract_diagnostic")`.
- Operador pode `RUST_LOG=info youtube-legend-cli --verbose ... 2>&1 | grep noteey_extract_diagnostic` para ver o dump.

### Benefícios da Solução
- Diagnóstico preciso sem re-executar manualmente.
- Distingue captcha (HTML widget CF Turnstile), empty page (`<body></body>`), render parcial (hero + 2 linhas vazias).
- CI pode usar `RUST_LOG=info` e parsear tracing para classificar falhas.

### Causa × Efeito
| Sintoma | Causa | Efeito |
|---------|-------|--------|
| `noteey_extract_diagnostic` no log com `cf-turnstile` | Captcha interstitial | Operador sabe que precisa trocar IP ou adicionar proxy |
| `noteey_extract_diagnostic` com `last_body_len: 0` | Página vazia | Degradação upstream confirmada |
| `noteey_extract_diagnostic` com header do site | Render parcial | DOM lento, retry pode funcionar |

### Como Solucionar
- Editar `EXTRACT_JS` para incluir `body: lastBody` no JSON de erro.
- Editar `drive_page` para ler `body`, slice 500 chars, emitir tracing warn.

### Próximo Passo
- ✅ Corrigido nesta sessão
- Operador pode diagnosticar via log estruturado sem re-executar


## GAP-AUD-2026-047 — noteey_to_text regression suite com 11 testes

### Problema
- `noteey_to_text` em `src/parse/mod.rs:140-173` foi adicionada em v0.3.2 sem cobertura de teste completa.
- Os primeiros commits da função tinham 1 teste básico (timestamp MM:SS simples) e 0 testes para edge cases.
- Regressões silenciosas: cap 50 MiB podia ser removido em refactor, normalização NFC podia quebrar, marcadores `[...]` podiam vazar para output.

### Consequências
- Refactor de `noteey_to_text` em mudanças futuras podia reintroduzir bugs sem detecção.
- Operador recebia output sujo sem perceber (marcadores `[Music]`, timestamps misturados, ou OOM em vídeos longos).

### Causa Raiz
- Falta de cultura de teste na adição da função. Os outros parsers (`srt_to_text`, `srv3_to_srt`, `json3_to_srt`) têm suítes robustas; `noteey_to_text` ficou atrasada.

### Solução
- 11 testes adicionados em `src/parse/mod.rs:245-339`:
  - `noteey_clean_strips_leading_timestamp`
  - `noteey_clean_handles_milliseconds`
  - `noteey_clean_handles_hh_mm_ss`
  - `noteey_clean_skips_empty_lines`
  - `noteey_clean_handles_accented_text`
  - `noteey_clean_normalizes_unicode_nfc`
  - `noteey_clean_skips_marker_lines`
  - `noteey_clean_rejects_empty_input`
  - `noteey_clean_rejects_whitespace_only`
  - `noteey_clean_respects_50mb_cap`
  - `noteey_clean_collapses_consecutive_empties`

### Benefícios da Solução
- Refactor seguro: qualquer mudança em `noteey_to_text` é validada automaticamente.
- Output consistente: operador sempre recebe texto limpo, sem timestamps ou marcadores.
- OOM prevenido: cap 50 MiB é testado explicitamente.

### Causa × Efeito
| Sintoma | Causa | Efeito |
|---------|-------|--------|
| Output com `[Music]` no meio | Marcadores não filtrados | UX ruim |
| Timestamp `00:01` no início do texto | Parser não strip prefix | Contrato quebrado |
| Crash em vídeo de 4h | Cap 50 MiB removido em refactor | OOM |
| Acentos quebrados | NFC normalization perdida | Encoding issue |

### Como Solucionar
- Adicionar suíte de testes inline em `src/parse/mod.rs::tests`.
- Cobrir: strip timestamp, normalização Unicode, marcadores, cap, edge cases (empty, whitespace, millis).

### Próximo Passo
- ✅ Corrigido nesta sessão
- Suíte robusta previne regressões


## GAP-AUD-2026-048 — findTranscriptRegion isola transcript pane excluindo header/nav/login

### Problema
- `EXTRACT_JS` em `provider_noteey.rs` originalmente lia `document.body.innerText` direto.
- O body inteiro da página inclui header (nav), hero, login button, page title — tudo isso vazaria para o output do parser Rust.
- Operador recebia: `youtube subtitle downloader Login Sign Up ... 00:00 taxa de juros ...` em vez de apenas `00:00 taxa de juros ...`.

### Consequências
- Output sujo com header do site misturado com transcript.
- Texto não é limpo mesmo após `noteey_to_text` remover timestamps.
- Pipeline de auditoria vê ruído do site que mascara o conteúdo real.

### Causa Raiz
- Seletor fraco: `document.body.innerText` é muito genérico.
- noteey.com SPA renderiza header DENTRO de `<body>` antes do transcript pane.

### Solução
- `provider_noteey.rs:78-93`: nova função `findTranscriptRegion()` em JS:
  1. Tentar seletor estável: `[data-transcript]`, `[class*="transcript" i]`, `[id*="transcript" i]`.
  2. Se nenhum casar, fallback heurístico: percorrer `div, section, article, main`, escolher o menor container com `≥3` timestamps.
  3. Retornar `innerText` do container selecionado ou `null` se nenhum casar.
- Rust side (`drive_page`) só processa body se `findTranscriptRegion` retornou não-nulo.

### Benefícios da Solução
- Output limpo: apenas o transcript pane é enviado ao parser Rust.
- Resiliente: heurística funciona mesmo quando DOM não tem seletor estável.
- Operador vê apenas o conteúdo real do vídeo.

### Causa × Efeito
| Sintoma | Causa | Efeito |
|---------|-------|--------|
| `Login` ou `Sign Up` no output | Header do site vazou | Ruído no output |
| `youtube subtitle downloader` no output | Page title vazou | Ruído no output |
| Transcript misturado com nav | `document.body.innerText` muito genérico | Parser recebe body inteiro |

### Como Solucionar
- Adicionar `findTranscriptRegion()` em `EXTRACT_JS`.
- Estratégia dupla: seletor estável + heurística ≥3 timestamps.
- Retornar `null` se nenhum casar (não fallback para `document.body.innerText`).

### Próximo Passo
- ✅ Corrigido nesta sessão
- Combinar com GAP-043 quando provider-noteey-http ficar viável para redundância


## GAP-AUD-2026-053 — Mutex poisoning degrada chain em providers headless

### Problema
- `provider_headless.rs:362-365` e `provider_noteey.rs:225-228` usam `.lock().map_err(|_| AppError::Internal("cache poisoned".into()))?` para acessar o Mutex<HashMap> de cache local.
- Se o Mutex for poisoned (panicked durante hold por qualquer motivo), a CLI retorna `AppError::Internal` (exit 70) em vez de recuperar automaticamente.
- Inconsistente com `provider/mod.rs:327, 345` que usa `unwrap_or_else(std::sync::PoisonError::into_inner)` para recovery automático.

### Consequências
- Falha transitória (panic em thread separada que segurava o Mutex) degrada o chain inteiro.
- Operador vê exit 70 `EX_SOFTWARE` em vez de exit 69 `EX_UNAVAILABLE` ou exit 66 `EX_NOINPUT` (mais apropriados).
- Recover automático via `into_inner()` é a prática padrão em Rust para Mutex que protege dados recuperáveis (cache local).

### Causa Raiz
- Padrão inconsistente: providers headless optaram por `map_err` enquanto provider chain optou por `unwrap_or_else(PoisonError::into_inner)`.
- Falta de decisão arquitetural clara sobre Mutex poisoning policy.

### Solução
- Mudar `provider_headless.rs:362-365` para:
  ```rust
  self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
      .insert(source_url.clone(), body.into_bytes());
  ```
- Mudar `provider_noteey.rs:225-228` similarmente.
- Adicionar teste de regressão em cada provider que valide recovery após poisoning (difícil de testar sem forçar panic).

### Benefícios da Solução
- Recovery automático em vez de exit 70.
- Consistência com `provider/mod.rs`.
- Operador vê erro mais apropriado (degraded ou unavailable).

### Causa × Efeito
| Sintoma | Causa | Efeito |
|---------|-------|--------|
| Exit 70 após falha transitória | Mutex poisoned | Chain termina prematuramente |
| `cache poisoned` no log | Mutex poisoned detectado | Operador não sabe que é recovery-able |
| Inconsistência entre providers | Padrão divergente | Manutenção confusa |

### Como Solucionar
- Substituir `.lock().map_err(...)?` por `.lock().unwrap_or_else(PoisonError::into_inner)` em ambos providers.
- Adicionar comentário explicando a escolha (recovery automático via into_inner).

### Próximo Passo
- v0.3.3 — severidade BAIXA (Mutex poisoning requer panic durante hold que ainda não foi observado).
- Não bloqueia release da v0.3.2.
- Fix é mecânico: 2 linhas para alterar.


## GAP-AUD-2026-054 — Chain consome `BrowserNotFound` em `saw_no_subtitle` e mascara falha de ambiente

### Problema × Consequências
O chain de providers retornava `AppError::NoSubtitle(NotPublished)` quando providers estáticos reportavam ausência confirmada de legendas E providers headless/noteey falhavam por falta de chrome no ambiente. O operador recebia `exit 66 EX_NOINPUT` sem informação de que chrome estava faltando, impedindo-o de instalar a dependência necessária para tentar a tier headless.

### Causa Raiz
`ProviderChain::fetch_subtitle` em `src/provider/mod.rs:404-411` trata `AppError::NoSubtitle(_)` como `degraded: false` e marca `saw_genuine_no_subtitle = true` na linha 462. Quando providers headless/noteey retornam `AppError::BrowserNotFound(_)` (chromiumoxide falha ao lançar o browser), o chain faz `continue` no bloco degraded (linha 459) **sem registrar o erro em `last_err`**. Como `BrowserNotFound` não é `NoSubtitle`, ele também não afeta `saw_genuine_no_subtitle`. Resultado: `last_err` contém apenas `NoSubtitle(...)` dos providers estáticos, e o chain retorna isso ao operador.

### Solução
GAP-AUD-2026-054 altera três pontos em `src/provider/mod.rs`:

1. `AppError::BrowserNotFound(_)` adicionado ao match arm degraded (linha 412-414), garantindo que o chain continue quando chrome está faltando.
2. `remember_failure` agora protege `BrowserNotFound` e `CaptchaChallenge` em adição a `RateLimited` (linha 75-92), permitindo que esses sinais sobrevivam quando um `NoSubtitle` posterior tenta sobrescrever.
3. O bloco degraded chama `remember_failure(&mut last_err, error)` antes do `continue` (linha 503-505), registrando o erro de ambiente em `last_err` sem marcar `saw_genuine_no_subtitle`.
4. Lógica final do chain (linha 514-522) verifica primeiro `last_err` para `BrowserNotFound` / `CaptchaChallenge` / `RateLimited` antes de colapsar para `NoSubtitle`. Esses sinais de ambiente vencem.

### Benefícios
- Operador recebe `exit 69 EX_UNAVAILABLE` com mensagem clara: "chromium/chrome not found: ... Set $CHROME or install chromium-browser / google-chrome".
- Diagnóstico preciso do motivo da falha: ambiente vs upstream confirmou ausência.
- `RateLimited` retém `Retry-After` em todos os cenários (EC-021 preservado).
- `CaptchaChallenge` agora também sobrevive ao chain (extensão simétrica do mesmo fix).

### Causa × Efeito
Causa: arm match do chain não distinguia entre NoSubtitle genuíno e BrowserNotFound de ambiente. Efeito: operador incapaz de diagnosticar falha de ambiente, recebia exit 66 sem dica.

### Como Solucionar
Edit em `src/provider/mod.rs:412-522`: adicionar BrowserNotFound ao match arm degraded, estender `remember_failure` para protegê-lo, registrar erro de ambiente em `last_err` mesmo no bloco degraded, e adicionar match arm final que prefere sinais de ambiente.

### Verificação
- `cargo test --features headless`: 250 passed, 0 failed.
- `cargo clippy --all-targets --features headless -- -D warnings`: zero warnings.
- E2E manual contra `https://youtu.be/TTEFo3XQYls` sem chrome instalado: `exit 69`, mensagem `chromium/chrome not found`.
- Dois testes em `src/provider/mod.rs` atualizados: `chain_treats_429_and_503_as_degraded_skips_both` (espera `RateLimited`, não `ProviderUnavailable`) e `chain_records_genuine_no_subtitle_after_two_degraded_providers` (espera `RateLimited` do 429 segundo, não `NoSubtitle` consolidado).

### Próximo Passo
Documentar a mudança no CHANGELOG.md em `[0.3.2]` subseção `### Fixed`. Auditar `provider/mod.rs` para garantir que nenhum outro `AppError` variante deveria ser degradado.
## GAP-AUD-2026-055 — chromiumoxide SingletonLock stale + mensagem de erro genérica

**Problema × Consequências × Causa Raiz × Solução × Benefícios × Causa × Efeito × Como Solucionar × Verificação × Próximo Passo**

### Problema
Chromiumoxide 0.9.1 usa por padrão `/tmp/chromiumoxide-runner/` como `user_data_dir`. Quando uma invocação anterior do CLI termina abruptamente (SIGKILL, OOM, crash do chrome), os symlinks `SingletonLock`, `SingletonCookie` e `SingletonSocket` permanecem apontando para PIDs mortos. A próxima invocação faz o chrome abortar com `Failed to create /tmp/chromiumoxide-runner/SingletonLock: Arquivo existe (17)` e exit 5376. A mensagem reportada pelo CLI era genérica: `Browser::launch failed: ... Set $CHROME or install chromium-browser / google-chrome`, o que induzia o operador a procurar um browser que já estava instalado.

### Consequências
- Operador sem informação sobre a causa real da falha.
- Impossível distinguir entre (a) browser faltando, (b) SingletonLock stale, e (c) incompatibilidade CDP entre chromiumoxide 0.9.1 e Chromium 149 (Fedora 44+).
- Logs ficam poluídos com mensagem falsa sobre `$CHROME`.

### Causa Raiz
- `BrowserConfig::builder()` em `provider_headless.rs:333` e `provider_noteey.rs:205` nunca setava `user_data_dir`, deixando o chromiumoxide escolher o default `/tmp/chromiumoxide-runner/`.
- Esse diretório é global no host, compartilhado entre providers e entre execuções.
- Não havia rotina de limpeza de singleton locks antes do `Browser::launch`.
- A mensagem de erro no `map_err` era a mesma para todas as variantes de falha.

### Solução
1. Novo helper `prepare_user_data_dir()` em `src/provider/stealth.rs:128` que:
   - Ancora o profile em `~/.cache/youtube-legend-cli/chrome-profile/` (XDG cache do projeto).
   - Faz sweep de `Singleton{Lock,Cookie,Socket}` órfãos antes do launch.
   - Probe de liveness via `kill -0 <pid>` em Unix (sem dependência nova).
2. `provider_headless.rs:333-351` e `provider_noteey.rs:205-213` agora chamam o helper e setam `user_data_dir` no `BrowserConfig`.
3. Mensagem de erro em `Browser::launch.map_err` distingue:
   - "Timeout while resolving websocket" / "Connection error" → CDP mismatch (chromiumoxide 0.9.1 não fala CDP com Chromium 149+).
   - Outras → mensagem genérica anterior.

### Benefícios
- Profile isolado por projeto (não colide com outros usuários do chromiumoxide).
- Lock stale é auto-curado a cada execução.
- Operador recebe mensagem acionável que aponta para a causa real.

### Causa × Efeito
| Causa | Efeito |
|-------|--------|
| BrowserConfig sem user_data_dir | chromiumoxide usa `/tmp/chromiumoxide-runner/` global |
| Crash anterior deixa SingletonLock órfão | Próxima invocação aborta com exit 5376 |
| Mensagem de erro genérica | Operador não sabe se é missing/incompat/stale |

### Como Solucionar
Aplicado via `atomwrite edit` em três arquivos (escopo cirúrgico). Sem novas dependências. O fix é backward-compatible: se `prepare_user_data_dir` falhar (improvável, mas defensivo), o builder cai para o comportamento anterior (sem `user_data_dir`).

### Verificação
- `cargo build --features headless` — zero warnings, 2.57s.
- `cargo clippy --all-targets --features headless -- -D warnings` — zero warnings.
- `cargo test --features headless` — 250+ testes passam, 0 falhando.
- E2E manual contra `https://youtu.be/wnZGZG1dRtI` (apaga cache antes): exit 69 com mensagem agora identifica CDP mismatch em vez de "Set $CHROME".

### Próximo Passo
- v0.3.3+ deve investigar se o `BrowserFetcher` consegue baixar a revisão pinned quando o ambiente tem rede.
- Considerar detecção proativa de CDP mismatch no `ensure_chrome()` (parar antes de tentar `Browser::launch`).
- Limpar diretórios órfãos `_tmp_yt-legend-offline-hit-*` em `~/.cache/` (acumulados em testes anteriores).

