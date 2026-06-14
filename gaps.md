# Gaps — youtube-legend-cli

Inventário vivo de problemas, lacunas, omissões e oportunidades de melhoria identificados durante auditorias incrementais do código contra as rules-rust do GraphRAG, o PRD v0.1.0 e a Constitution (PRINC-001 a PRINC-015).

## Formato Canônico

Cada gap segue rigorosamente o template problema x consequências x causa raiz x solução x benefícios x como solucionar com relações causa x efeito explícitas. Gaps ainda nao solucionados ficam em estado ABERTO. Gaps solucionados migram para o registro de verificação.

---
## GAP-007 — Sigilo vazado pela API pública do crate via pub mod secret_endpoints

- **Data de identificação:** 2026-06-13
- **Severidade:** CRÍTICA
- **Status:** CORRIGIDO 2026-06-14
- Detectado por: Auditoria rules-rust contra PRD PRINC da Constitution e decisão de sigilo
- Arquivos afetados: src/lib.rs:113, src/secret_endpoints.rs, src/crypto.rs:14, src/bin/snapshot.rs:17-19

### Problema

O módulo src/secret_endpoints.rs e exposto como pub mod secret_endpoints no src/lib.rs:113, e os identificadores nele sao declarados como pub const PROVIDER_A_PRIMARY_HOST: &str = "downsub.com";. Esta combinação torna as constantes acessiveis via youtube_legend_cli::secret_endpoints::PROVIDER_A_PRIMARY_HOST para qualquer consumidor da crate. O src/bin/snapshot.rs:17-19 inclusive importa esses identificadores via caminho público youtube_legend_cli::secret_endpoints::{...}, evidencia material de que o segredo e alcançável pela superficie pública. Quando a crate for públicada em crates.io, o docs.rs renderizara o módulo inteiro em HTML público, vazando hosts, paths, tokens e o OBFUSCATED_PASSWORD que o src/crypto.rs:14 reexporta como pub const OBFUSCATED_PWD.

### Consequências do Problema

- Vazamento permanente dos hosts dos provedores (downsub.com, www.downloadyoutubesubtitles.com, mywatchtones.com, get-info.downsub.com, subtitle.downsub.com) na documentação pública de docs.rs.
- Vazamento do OBFUSCATED_PASSWORD que serve como chave PBKDF2 do provider B, possibilitando engenharia reversa do esquema de assinatura de request.
- Quebra do sigilo declarado na Constitution e na decisão de sigilo de 2026-06-07, que proibe divulgação de identificadores concretos em código-fonte fora de src/secret_endpoints.rs E em superficie pública de API.
- Aceleração de bloqueio por ASN compartilhado se alguem descobrir os hosts via docs.rs e denuncia-los aos provedores como abuso de scraping.
- Risco jurídico se os Termos de Uso dos provedores proibirem divulgação dos endpoints técnicos.
- Falha de auditoria automatizada quando a politica de sigilo for checada via rg em artefatos públicados.

### Causa Raiz

Dois erros arquiturais acumulados durante a criação do módulo de sigilo:

1. Visibilidade do módulo: o pub mod secret_endpoints em src/lib.rs:113 foi escrito seguindo o padrão idiomatico de módulos Rust, mas sem considerar que o crate e distribuido em crates.io com docs.rs ativo. O correto seria pub(crate) mod secret_endpoints.

2. Visibilidade dos itens: os pub const dentro de secret_endpoints.rs foram mantidos pub em vez de pub(crate) const porque o autor pensou em pub como visivel dentro do projeto, sem distinguir dentro do crate de fora do crate na superficie públicada.

A causa raiz declarada explícitamente: falta de mentalidade de pub(crate) como padrão para qualquer identificador que nao precise ser consumido por terceiros, somada a ausencia de gate de CI que falhe o build se cargo public-api ou cargo doc detectar o módulo na superficie pública.

### Solução

Aplicar visibilidade pub(crate) em duas camadas:

1. src/lib.rs:113: alterar pub mod secret_endpoints para pub(crate) mod secret_endpoints. O módulo deixa de aparecer na API pública e em docs.rs.
2. src/secret_endpoints.rs: alterar todas as declaracoes pub const para pub(crate) const. Apenas os itens que outros módulos do crate consomem (crate::provider, crate::crypto, crate::bin::snapshot) precisam de visibilidade intra-crate.
3. src/crypto.rs:14: alterar pub const OBFUSCATED_PWD para pub(crate) const OBFUSCATED_PWD, ou remover a reexportacao ja que encrypt_token usa o OBFUSCATED_PASSWORD diretamente.
4. Gate de CI: adicionar ao CI um passo rg -c pub mod secret_endpoints src/lib.rs que falha se diferente de 0 (forcar pub(crate)).

### Benefícios da Solução

- Sigilo preservado: os identificadores ficam acessiveis apenas dentro do binario compilado, jamais no HTML de docs.rs.
- Conformidade com Constitution: alinha com a politica NAO públicar nomes de provedores em README, CHANGELOG, código-fonte ou issues.
- Conformidade com PRD secao 13 Constraints: o stack aprovada termina com license MIT OR Apache-2.0; o sigilo so e efetivo se o código compilado nao vazar identificadores, e pub(crate) garante isso.
- Deteccao precoce de regressao: o gate de CI pega futuros descuidos de qualquer contribuidor que tente voltar o pub por preguica.
- Zero impacto de performance: pub(crate) e puramente um modificador de visibilidade em tempo de compilação; nao muda o binario gerado.
- Compatibilidade retroativa: como o módulo nunca foi documentado públicamente (e o #![allow(missing_docs)] confirma), nenhum consumidor externo legitimo usa esses identificadores.

### Como Solucionar (passos verificáveis)

1. Capturar baseline do problema: atomwrite --workspace . read --stat src/lib.rs para obter checksum. Registrar RG_SIGILO=rg -c ^pub mod secret_endpoints src/lib.rs e validar que retorna 1 antes da correcao.
2. Editar src/lib.rs:113: trocar pub mod secret_endpoints por pub(crate) mod secret_endpoints via atomwrite --workspace . edit.
3. Editar src/secret_endpoints.rs: trocar todos os pub const por pub(crate) const.
4. Editar src/crypto.rs:14: trocar pub const OBFUSCATED_PWD por pub(crate) const OBFUSCATED_PWD ou remover a reexportacao.
5. Validar compilação: cargo build deve passar com 0 warnings.
6. Validar testes: cargo test deve manter 77 passing + 4 ignored.
7. Validar clippy: cargo clippy --all-targets -- -D warnings deve contínuar 0.
8. Validar sigilo em artefato gerado: cargo doc e em seguida rg nos artefatos gerados deve retornar 0.
9. Persistir no GraphRAG: usar o subcomando de persistencia da CLI com entidades e relações curadas.
10. Adicionar ao CI gate: criar script check-sigilo.sh que executa rg -c ^pub mod secret_endpoints src/lib.rs e falha se diferente de 0.

### Causa x Efeito

- pub mod secret_endpoints causa pub const PROVIDER_* alcançável pela API pública.
- pub mod secret_endpoints causa renderizacao em docs.rs ao públicar em crates.io.
- cargo doc em CI causa geracao de target/doc/youtube_legend_cli/secret_endpoints/index.html com identificadores reais.
- Constituição PRINC NAO públicar nomes causa obrigacao de sigilo absoluto.
- pub mod secret_endpoints contradiz Constituição PRINC.
- OBFUSCATED_PASSWORD em pub const causa reexportacao em crypto::OBFUSCATED_PWD.
- A ausencia de gate de CI permite que regressões futuras passem despercebidas.
- pub(crate) mod secret_endpoints fixa o vazamento.
- CI gate de sigilo fixa futuras regressões.

### Anti-pattern Categorizado

- Categoria: rules-rust-código-ingles-internacionalizacao + rules-rust-proibicao-hardcode + decisão-sigilo-provedores-youtube-legend
- Regras violadas: OBRIGATORIO de aplicar pub(crate) por padrão; PROIBIDO de exportar identificadores sensiveis em superficie pública de biblioteca.
- Ferramentas que DEVEM ser usadas no fix: atomwrite (com --expect-checksum e --dry-run), rg (auditoria), cargo doc (geracao de docs), persistencia via CLI GraphRAG, context7 (consultar Rust API Guidelines sobre pub(crate)).

---

## Historico de Gaps Anteriores (referencia)

- GAP-001 a GAP-006 (auditoria 2026-06-11): documentados na memoria youtube-legend-cli-auditoria-rules-2026-06-11. Todos CORRIGIDOS.
---

## GAP-008 — NFR-005 (offline com cache hit) sem teste de integração

- **Data de identificação:** 2026-06-13
- **Severidade:** ALTA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Auditoria rules-rust contra PRD seção 7 (NFR-005)
- **Arquivos afetados:** `tests/integration/corpus.rs`, `Cargo.toml [dev-dependencies]`, `src/cache.rs`

### Problema

O PRD v0.1.0 declara no NFR-005 que o sistema DEVE funcionar offline após compilação, exceto pelo acesso HTTP aos provedores, com métrica explícita: execução com rede bloqueada após compilação completa, exit code 0 em URL em cache. Nenhum teste de integração exercita este cenário. A suíte atual depende de rede real (4 testes com `#[ignore]` aguardam fixture ao vivo).

### Consequências do Problema

- Impossível afirmar de forma reprodutível que o NFR-005 está satisfeito.
- Regressões futuras em `cache.rs::read_cache` ou em `commands::extract::run` não são detectadas por CI.
- O contrato com o usuário (Danilo) de uso offline com cache hit é verbal, não verificável.
- A métrica do NFR-005 (exit code 0 com rede bloqueada) nunca foi coletada.

### Causa Raiz

A suíte de testes foi desenhada para validar caminhos de sucesso ao vivo e não para validar o caminho de cache hit isolado. O `wiremock` está disponível em `Cargo.toml` como dev-dependency mas não é usado para simular um cache populado + rede bloqueada.

### Solução

Adicionar `tests/integration/offline_cache.rs` com dois casos: (1) `#[tokio::test] async fn nfr_005_offline_cache_hit_returns_zero()` que pré-popula o diretório de cache, executa o binário via `assert_cmd::Command::cargo_bin("youtube-legend-cli")` com a flag `--no-cache=false` e `--cache-ttl=24`, captura stdout e exit code, valida que sai com 0 e o conteúdo do cache. (2) `#[tokio::test] async fn nfr_005_offline_cache_miss_returns_five()` que valida o caminho oposto: cache vazio + rede off (simulável via `wiremock` retornando connection refused) → exit code 5.

### Benefícios da Solução

- NFR-005 passa de promessa verbal para contrato verificável em CI.
- Regressões em `cache.rs` são detectadas em segundos.
- A métrica do PRD (exit 0 offline) tem gate automatizado.
- Cobertura de testes aumenta em 2 cenários novos.

### Como Solucionar (passos verificáveis)

1. Criar `tests/integration/offline_cache.rs` com 2 testes conforme acima.
2. Adicionar `wiremock` para o teste de cache miss com server rodando em `127.0.0.1:0` que retorna connection refused via abort.
3. Validar via `timeout 90 cargo test --test offline_cache` que ambos passam.
4. Adicionar `cargo test --test offline_cache` ao CI como gate obrigatório.
5. Persistir fix no GraphRAG como `youtube-legend-cli-nfr-005-test-2026-06-13`.

### Causa x Efeito

- Ausência de teste de integração para cache hit **causa** NFR-005 não verificável.
- `wiremock` já em dev-dependencies **permite** escrever o teste sem adicionar dependência nova.
- Regressão em `cache.rs::read_cache` **causa** falha do NFR-005 em produção sem detecção prévia.
- `#[ignore]` nos 4 testes existentes **bloqueia** a verificação regular de NFR-005 em CI.
- Teste de integração com `assert_cmd` **fixa** a verificabilidade do NFR-005.
- CI gate de `cargo test --test offline_cache` **fixa** futuras regressões.

### Anti-pattern Categorizado

- Categoria: `rules-rust-testes-sem-travar` + `rules-rust-cli-stdin-stdout-testes-seguranca`.
- Regras violadas: OBRIGATÓRIO de testar NFRs em CI, não apenas em ambiente de desenvolvedor.

---

## GAP-009 — EC-015 (dedup de URLs em modo batch) não implementado

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** CORRIGIDO 2026-06-13 (drift documental — implementação HashSet em src/commands/batch.rs:14,42-45)
- **Detectado por:** Auditoria rules-rust contra PRD seção 11 (EC-015)
- **Arquivos afetados:** `src/commands/batch.rs`

### Problema

O PRD v0.1.0 declara em EC-015 que o sistema DEVE deduplicar URLs duplicadas em modo batch e processar cada URL única uma única vez. A implementação atual em `src/commands/batch.rs::run` itera sobre `urls` diretamente sem aplicar nenhum `HashSet<String>` de deduplicação. Entrada com 10 linhas onde 5 são duplicadas resulta em 5 downloads redundantes com 5 traces de tracing.

### Consequências do Problema

- Desperdício de banda e quota dos provedores.
- Latência inflada para o usuário que cola acidentalmente a mesma URL duas vezes.
- Logs de progresso poluídos com eventos repetidos.
- Contadores de `--json` batch emitem 10 objetos com mesmo `video_id`.
- Comportamento diverge silenciosamente da especificação do PRD sem alerta.

### Causa Raiz

Otimização prematura não feita: o autor assumiu que input bem-formado não tem duplicatas e não implementou a defesa do EC-015. Falta de teste de regressão que capturasse o caso.

### Solução

Em `src/commands/batch.rs::run`, antes do loop de processamento:

```rust
let mut seen = std::collections::HashSet::new();
let unique_urls: Vec<String> = urls.into_iter().filter(|u| seen.insert(u.clone())).collect();
```

Emitir um `tracing::info!(event = "dedup", removed = urls.len() - unique_urls.len())` quando há duplicatas. No modo `--json`, manter o array com `index` refletindo a posição da URL única (não a posição original).

### Benefícios da Solução

- Conformidade literal com EC-015.
- Economia de banda: usuário que duplica URLs vê latência proporcional ao trabalho real.
- Logs de progresso mais limpos.
- Output JSON determinístico (sem índices duplicados).

### Como Solucionar (passos verificáveis)

1. Adicionar `use std::collections::HashSet;` em `src/commands/batch.rs`.
2. Inserir o filtro de dedup na função `run` antes do loop principal.
3. Adicionar `#[test] fn batch_dedups_repeated_urls()` que valida que 3 URLs iguais produzem 1 download.
4. Validar `cargo test` mantém 77+ passing + 4 ignored.
5. Persistir fix no GraphRAG.

### Causa x Efeito

- Ausência de HashSet no loop **causa** downloads redundantes.
- PRD EC-015 explícito **causa** obrigação contratual de dedup.
- Logs de tracing sem campo `dedup_removed` **causa** poluição de observabilidade.
- HashSet::insert em filtro **fixa** a redundância.
- Teste `batch_dedups_repeated_urls` **fixa** regressão futura.

### Anti-pattern Categorizado

- Categoria: `rules-rust-eficiencia-e-performance` + `rules-rust-economia-de-recursos`.
- Regras violadas: OBRIGATÓRIO de implementar ECs documentados no PRD antes do MVP.

---

## GAP-010 — NFR-007 (robots.txt compliance) não implementado

- **Data de identificação:** 2026-06-13
- **Severidade:** ALTA (Constitution PRINC explícito: "Compliance: observar robots.txt dos provedores")
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Auditoria rules-rust contra PRD seção 13 Constraints + Constitution
- **Arquivos afetados:** `src/provider/provider_a.rs`, `src/provider/provider_b.rs`, `Cargo.toml [dependencies]`

### Problema

O PRD v0.1.0 declara em NFR-007 que o sistema DEVE respeitar robots.txt dos provedores, com métrica: request com User-Agent identificável e rate limit 1 request por segundo por provedor. A Constitution declara em Constraints: "Compliance: observar robots.txt dos provedores". Nenhum código implementa parser de robots.txt. O rate limit de 1 req/s está implementado em `src/provider/mod.rs::ProviderChain`, mas o robots.txt não é consultado antes de cada request.

### Consequências do Problema

- Violação literal de robots.txt dos provedores sem detecção.
- Bloqueio imediato de ASN compartilhado se o robots.txt dos provedores contiver `Disallow: /` para o user-agent do projeto.
- Risco jurídico aumentado: scraping que ignora robots.txt pode configurar violação de CFAA nos EUA.
- Inconsistência entre a regra da Constitution e o código.

### Causa Raiz

Falta de parsing de robots.txt. O código apenas respeita rate limit de 1 req/s e usa User-Agent identificável, mas não consulta o arquivo `robots.txt` do provedor antes de fazer scraping.

### Solução

Adicionar dependência `robotstxt = "0.3"` (crate Rust mantida pelo Google). Em `src/provider/provider_a.rs::fetch_page` e `src/provider/provider_b.rs::fetch_page`, antes de cada GET, consultar `https://<host>/robots.txt` uma vez por processo (cachear resultado), parsear via `robotstxt::RobotsTxt::parse`, e verificar se o path pretendido é permitido para o User-Agent `youtube-legend-cli/0.1.0`. Se bloqueado, retornar `AppError::ProviderUnavailable` com mensagem `robots.txt disallows this path`.

### Benefícios da Solução

- Conformidade legal e ética com o robots.txt dos provedores.
- Detecção precoce de bloqueio antes de gastar request HTTP.
- Mensagem de erro específica para o usuário (vs 403 opaco).
- Coerência com a Constitution.

### Como Solucionar (passos verificáveis)

1. Adicionar `robotstxt = "0.3"` em `Cargo.toml [dependencies]`.
2. Criar `src/provider/robots.rs` com função `check_allowed(host, path, ua) -> AppResult<()>` que cachea o resultado em `OnceLock`.
3. Chamar `check_allowed` em cada método `fetch_page` de provider_a e provider_b antes do GET.
4. Adicionar teste com `wiremock` que serve robots.txt com `Disallow: /` e valida que o provider retorna `AppError::ProviderUnavailable`.
5. Validar `cargo build` + `cargo test` mantêm 77+ passing.

### Causa x Efeito

- Ausência de parser robots.txt **causa** violação de NFR-007.
- Constitution Constraints **causa** obrigação legal/ética.
- Scraping não-compliant **causa** bloqueio de ASN.
- Crate `robotstxt` em deps **fixa** o gap.
- Teste com wiremock + Disallow: / **fixa** regressão.

### Anti-pattern Categorizado

- Categoria: `rules-rust-web_scraping_http_html_json_csv` + `rules-rust-codigo-ingles-internacionalizacao`.
- Regras violadas: OBRIGATÓRIO de respeitar robots.txt em qualquer scraping ético.

---

## GAP-011 — Falta `#[instrument]` em fronteiras públicas da API interna

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** EXPANDIDO 2026-06-13
- **Detectado por:** Auditoria rules-rust-tracing-instrument-spans-macros
- **Arquivos afetados:** `src/provider/mod.rs`, `src/provider/provider_a.rs`, `src/provider/provider_b.rs`, `src/commands/extract.rs`, `src/commands/batch.rs`, `src/cache.rs`, `src/parse/mod.rs`

### Problema

A regra `rules-rust-tracing-instrument-spans-macros` declara como OBRIGATÓRIO: instrumentar fronteiras públicas da API interna, instrumentar handlers de request em servidores, instrumentar operações de I/O com latência relevante, instrumentar tarefas de longa duração em workers, nomear span com verbo de ação no imperativo. Nenhum `#[tracing::instrument]` ou `#[tracing::instrument(skip(self), fields(...))]` está presente em nenhum arquivo de `src/`. Toda a observabilidade via `tracing::info!`, `tracing::warn!`, `tracing::error!` é ad-hoc, sem spans nomeados correlacionáveis.

### Consequências do Problema

- Impossível correlacionar logs de uma única operação de fetch a um span de request.
- OpenTelemetry tracing (mencionado em rules-rust-config-observabilidade) ficaria sem spans de borda.
- Debugging de fluxos longos (batch de 100 URLs) exige grep manual.
- Em produção, identificar qual fetch está lento exige logs de timing manuais.

### Causa Raiz

Início do projeto focado em funcionalidade (FR-001 a FR-015) sem planejamento de observabilidade estruturada. O `tracing` foi adicionado tarde apenas para satisfazer o gap de log de erro (GAP-001 da auditoria 2026-06-11), mas sem aplicar a macro `#[instrument]`.

### Solução

Adicionar `#[tracing::instrument(level = "debug", skip(self), fields(video_id = %video_id))]` em cada método público de:
- `Provider::fetch_subtitle` em `src/provider/mod.rs:101`
- `ProviderA::fetch_subtitle` em `src/provider/provider_a.rs:50`
- `ProviderB::fetch_subtitle` em `src/provider/provider_b.rs:50`
- `extract::run` em `src/commands/extract.rs:30`
- `batch::run` em `src/commands/batch.rs:40`
- `cache::read_cache` em `src/cache.rs:120`
- `cache::write_cache` em `src/cache.rs:140`
- `parse::srt_to_text` em `src/parse/mod.rs:50`
- `parse::video_id::extract_video_id` em `src/parse/video_id.rs:30`

### Benefícios da Solução

- Spans nomeados correlacionáveis em `--verbose --format json`.
- Pronto para integração com OpenTelemetry export sem reescrita.
- Cumprimento literal da regra rules-rust-tracing.
- Debugging estruturado: cada `start`/`end` de span aparece nos logs JSON.

### Como Solucionar (passos verificáveis)

1. Adicionar `#[tracing::instrument]` em cada item da lista acima via `atomwrite edit --old --new`.
2. Validar `cargo build` 0 warnings.
3. Validar `cargo test --features none` mantém 77+ passing.
4. Validar `cargo test -- --nocapture` em um teste de extract mostra spans com nomes `extract`, `fetch_subtitle`, `read_cache`.
5. Persistir fix no GraphRAG.

### Causa x Efeito

- Ausência de `#[instrument]` **causa** logs sem correlação por span.
- Operações async concorrentes **causa** dificuldade de rastreamento.
- `#[tracing::instrument(level = "debug", ...)]` **fixa** a observabilidade estruturada.
- OpenTelemetry export **causa** demanda futura por spans correlacionados.

### Anti-pattern Categorizado

- Categoria: `rules-rust-tracing-instrument-spans-macros`.
- Regras violadas: OBRIGATÓRIO de `#[instrument]` em fronteiras públicas de API interna.

---

## GAP-012 — Testes de provider A e B com wiremock ausentes (apenas network real)

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Auditoria rules-rust-testes-sem-travar + rules-rust-cli-stdin-stdout-testes-seguranca
- **Arquivos afetados:** `src/provider/provider_a.rs`, `src/provider/provider_b.rs`, `tests/integration/`

### Problema

Os 4 testes com `#[ignore]` em `tests/integration/rss.rs` e `tests/integration/corpus.rs` dependem de rede real contra provedores third-party. Não existem testes com `wiremock` que simulem respostas HTTP dos provedores. Quando o provedor está offline (como aconteceu no incidente `youtube-legend-cli-endpoint-drift-requires-browser-2026-06-11`), os testes inteiros não rodam em CI.

### Consequências do Problema

- Cobertura efetiva dos providers em CI é ZERO.
- Regressões em `ProviderA::fetch_subtitle` e `ProviderB::fetch_subtitle` não são detectadas em PRs.
- Refatorações de regex ou seletores CSS quebram em produção sem aviso prévio.
- O gate `cargo test` passa com 0 testes para os providers em CI.

### Causa Raiz

Os provedores têm estrutura HTML mutável (RN-001 do PRD: "Mudança de estrutura HTML dos provedores") e o autor do código assumiu que snapshots de HTML real eram mais úteis que mocks. Isso é parcialmente verdadeiro para detectar drift, mas deve coexistir com testes mockados que rodem sempre em CI.

### Solução

Criar `tests/integration/provider_a_wiremock.rs` e `tests/integration/provider_b_wiremock.rs` com 5 testes cada:
- `provider_a_fetches_subtitle_success` — wiremock retorna HTML válido + SRT válido, provider retorna Ok
- `provider_a_returns_invalid_url_on_400` — wiremock retorna 400, provider retorna InvalidUrl
- `provider_a_returns_no_subtitle_on_404` — wiremock retorna 404, provider retorna NoSubtitle
- `provider_a_returns_rate_limited_on_429` — wiremock retorna 429 com Retry-After, provider retorna RateLimited com retry_after_secs
- `provider_a_returns_provider_unavailable_on_500` — wiremock retorna 500, provider retorna ProviderUnavailable

Mesma matriz para provider_b, com fixtures HTML/SRT realistas (mínimo 1 KB cada).

### Benefícios da Solução

- Cobertura efetiva dos providers em CI sobe de 0 para 10 cenários.
- Detecção precoce de regressões em mudanças de regex ou seletor CSS.
- Testes determinísticos rodam em < 1s total.
- Gate `cargo test` passa a ter garantia real de cobertura dos providers.

### Como Solucionar (passos verificáveis)

1. Criar fixtures HTML/SRT em `tests/fixtures/provider_a/` e `tests/fixtures/provider_b/`.
2. Criar `tests/integration/provider_a_wiremock.rs` e `tests/integration/provider_b_wiremock.rs` com 5 testes cada.
3. Validar `cargo test --test provider_a_wiremock` e `cargo test --test provider_b_wiremock` passam sem `#[ignore]`.
4. Validar `cargo test` total agora é 87 passing (77+10), 4 ignored.
5. Persistir fix no GraphRAG.

### Causa x Efeito

- 4 testes `#[ignore]` **causa** cobertura zero em CI para providers.
- `wiremock` em dev-dependencies **permite** escrever testes sem rede.
- Mudança de HTML do provedor (RN-001) **causa** quebra silenciosa em produção.
- 10 testes mockados **fixam** detecção em segundos.
- Gate de CI `cargo test --test provider_*_wiremock` **fixa** regressões.

### Anti-pattern Categorizado

- Categoria: `rules-rust-testes-sem-travar` + `rules-rust-cli-stdin-stdout-testes-seguranca`.
- Regras violadas: OBRIGATÓRIO de testar providers com wiremock, não apenas com rede real.

---

## Resumo dos Gaps Abertos (substituído — ver tabela atualizada mais abaixo)

Esta tabela de 2026-06-13 está OBSOLETA. A tabela canônica vigente está na seção
**Resumo Atualizado dos Gaps (2026-06-14 02:00 BRT pós-turno)** logo após a seção
**Gaps Falsos Positivos Identificados**. Mantida apenas como referência histórica.
| ID | Severidade | Descrição resumida | Status (histórico 2026-06-13) |
|----|------------|--------------------|-------------------------------|
| GAP-007 | CRÍTICA | Sigilo vazado via pub mod secret_endpoints | ABERTO |
| GAP-008 | ALTA | NFR-005 (offline + cache hit) sem teste | ABERTO |
| GAP-009 | MÉDIA | EC-015 (dedup de URLs em batch) não implementado | ABERTO |
| GAP-010 | ALTA | NFR-007 (robots.txt compliance) não implementado | ABERTO |
| GAP-011 | MÉDIA | Falta `#[instrument]` em API interna | ABERTO |
| GAP-012 | MÉDIA | Testes de provider com wiremock ausentes | ABERTO |

## Pendências de Auditoria Original (histórico — resolvido em GAPs posteriores)

Lista de itens mencionados em auditorias anteriores que ficaram fora do escopo desta varredura. **MANTIDA APENAS COMO REFERÊNCIA HISTÓRICA**: cada item foi subsequentemente endereçado em outros GAPs do inventário canônico (vide tabela `Resumo Atualizado dos Gaps (2026-06-14 02:00 BRT pós-turno)` mais adiante neste arquivo). A frase "ficaram fora do escopo" está OBSOLETA.

- NFR-002 (RSS < 100 MB) — RESOLVIDO em **GAP-020** (tests/integration/rss.rs:MAX_RSS_KIB 100*1024)
- NFR-003 (binário release < 20 MB) — VERIFICADO PASS em **GAP-021** (8,4 MB release)
- NFR-008 (cargo install sem deps) — RESOLVIDO em **GAP-022** (.github/workflows/ci.yml cross-compile: 6 targets)
- Constitution PRINC-009 (versão síncrona) — FALSO POSITIVO em **GAP-032** (build.rs depende de chrono, manter [build-dependencies])
- EC-024 (heurísticas múltiplas para HTML drift) — RESOLVIDO em **GAP-023** (provider_a.rs:253-354 JSON-LD VideoObject)
- Cross-compile a targets darwin — RESOLVIDO em **GAP-024** (ci.yml:108-111 x86_64-apple-darwin + aarch64-apple-darwin com continue-on-error)
- 26 arquivos `.bak` em `src/` — RESOLVIDO: `find src -name "*.bak*"` retorna 0 hits (CHANGELOG v0.2.6 Removed)

---

## GAP-013 — GAP-009 (dedup batch) JÁ IMPLEMENTADO mas marcado ABERTO em gaps.md

- **Data de verificação:** 2026-06-13
- **Severidade:** BAIXA (apenas atualização de status)
- **Status:** CORRIGIDO (mas não refletido em gaps.md)
- **Detectado por:** Reauditoria 2026-06-13 pós-compactação
- **Arquivos:** `src/commands/batch.rs:14,42-45`

### Problema

O GAP-009 declarava que `src/commands/batch.rs` não tinha dedup. Reauditoria mostra que `HashSet` JÁ está implementado em `src/commands/batch.rs:14` (`use std::collections::HashSet;`) e `src/commands/batch.rs:42-45`:

```rust
let mut seen: HashSet<String> = HashSet::new();
let unique_urls: Vec<String> = urls.into_iter()
    .filter(|u| seen.insert(u.clone()))
    .collect();
```

A documentação em gaps.md:156-216 está defasada. A implementação atende ao EC-015 do PRD.

### Consequência

- Confusão ao revisar a auditoria: o gap aparece como ABERTO mas código está conforme.
- Decisor de prioridade pode investir esforço em código já pronto.

### Causa Raiz

Auditoria incremental registrada gap antes de verificar se correção já havia sido aplicada entre 2026-06-11 (auditoria inicial) e 2026-06-13 (esta varredura).

### Solução

Atualizar gaps.md: GAP-009 → Status `CORRIGIDO 2026-06-13`. Adicionar referência ao commit ou nota inline em src/commands/batch.rs que aponte para o gap histórico.

### Como Solucionar (passos verificáveis)

1. Localizar GAP-009 em gaps.md:156-216.
2. Substituir `**Status:** ABERTO` por `**Status:** CORRIGIDO 2026-06-13`.
3. Adicionar seção "Verificação" listando `src/commands/batch.rs:14,42-45` como evidência.
4. Persistir nota no GraphRAG.

### Causa x Efeito

- Status desatualizado em gaps.md **causa** retrabalho de leitura.
- Implementação já presente em batch.rs **fixa** quando status é corrigido.
- Reauditoria pós-compactação **fixa** drift documental.

---

## GAP-014 — GAP-011 (#[instrument]) PARCIALMENTE IMPLEMENTADO mas marcado ABERTO em gaps.md

- **Data de verificação:** 2026-06-13
- **Severidade:** BAIXA (apenas expansão de cobertura)
- **Status:** PARCIALMENTE CORRIGIDO
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/commands/batch.rs:37`, `src/commands/extract.rs:29`, `src/bin/snapshot.rs:48`

### Problema

GAP-011 declarava que NENHUM `#[tracing::instrument]` estava presente. Reauditoria mostra que 3 instrumentações JÁ EXISTEM:

```rust
// src/commands/batch.rs:37
#[instrument(skip(cli, chain), fields(total))]

// src/commands/extract.rs:29
#[instrument(skip(cli, chain), fields(video_id, language = %language_to_str(cli.lang)))]

// src/bin/snapshot.rs:48
#[tracing::instrument(skip(args), fields(corpus = %args.corpus.display(), output_dir = %args.output_dir.display(), timeout_s = args.timeout))]
```

Mas ainda FALTAM instrumentações em:
- `src/provider/mod.rs::Provider::fetch_subtitle` (trait method)
- `src/provider/provider_a.rs::fetch_subtitle`
- `src/provider/provider_b.rs::fetch_subtitle`
- `src/cache.rs::read_cache`, `write_cache`
- `src/parse/mod.rs::srt_to_text`
- `src/parse/video_id.rs::extract_video_id`

### Consequência

- Status ABERTO reflete só parte da verdade (3/9 instrumentações).
- Spans de provider fetch não são correlacionáveis.
- A OpenTelemetry export ficaria parcial.

### Solução

Expandir `#[instrument]` para os 6 itens faltantes. Atualizar GAP-011 em gaps.md para Status `PARCIALMENTE CORRIGIDO` e listar implementado vs pendente.

### Causa x Efeito

- Instrumentação parcial **causa** cobertura observabilidade incompleta.
- Implementação nos comandos **fixa** observabilidade de UX.
- Faltam nos providers **causa** quebra de correlação HTTP→comando.
- `#[instrument]` em todos os 9 pontos **fixa** a cobertura completa.

---

## GAP-015 — 6 `Regex::new` em src/provider/provider_b.rs para parsing de JavaScript inline (não HTML)

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** FALSO POSITIVO CONFIRMADO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/provider/provider_b.rs:153,155,160,226,235,261`

### Problema

PRINC-007 declara que HTML parsing DEVE ser via scraper. Mas provider_b.rs usa 6 `Regex::new` para extrair variáveis JavaScript (`var tutoken=...`, `var htoken=...`, `["'](/[A-Za-z0-9_./-]*\.php)["']`, URLs `.srt/.txt/.vtt`).

### Análise

JavaScript inline (`<script>var tutoken='...';var htoken='...'</script>`) é texto simples dentro de um `<script>` tag, não HTML estruturado. PRINC-007 diz "HTML parsing via scraper" mas o alvo é JavaScript text content, não DOM tree.

### Consequência (se considerado violação)

- Refator para scraper é overkill: scraper.parse_html(script_content) não ajuda a extrair `tutoken='...'`.
- Regex é a ferramenta idiomática para extração de tokens em JS inline.

### Recomendação

GAP-015 NÃO é gap real — é uso correto de regex para extração de tokens JS. Documentar como decisão consciente em constitution ou spec técnica.

### Causa x Efeito

- JavaScript inline **causa** necessidade de regex (não scraper).
- PRINC-007 textual **causa** confusão sem nota explicativa.
- Documentar como decisão consciente **fixa** o falso positivo.

---

## GAP-016 — PRINC-003 (anyhow::Result) NÃO seguido — código usa AppError via thiserror

- **Data de verificação:** 2026-06-13
- **Severidade:** INFO (decisão consciente)
- **Status:** DECISÃO ARQUITETURAL
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/error.rs`

### Problema

PRINC-003: "Tratamento de erro com anyhow::Result em binários e operador ? para propagação".

Código usa `AppResult<T>` definido em `src/error.rs` baseado em `thiserror` com enum `AppError` e 14 variants tipadas. `anyhow` está disponível mas não é usado.

### Análise

Vantagem de `AppError` tipado vs `anyhow`:
- `exit_code()` determinístico por variant (mapping explícito para 2/3/4/5/6/7)
- `Display` em português brasileiro (PRINC-002) por variant
- `NoSubtitleReason` enum interno
- Matchable exaustivo em testes

Vantagem de `anyhow`:
- Menos boilerplate
- Contexto via `.context("...")`
- `Result<T>` em qualquer lugar

Decisão: `AppError` é mais explícito e mapeia melhor para exit codes. Manter.

### Solução

Nenhuma. Documentar como decisão consciente em constitution adicionando nota: "PRINC-003 é OBRIGATÓRIO usar thiserror/thiserror com enum tipado que mapeia para exit codes; anyhow é PROIBIDO pois não permite match exaustivo em testes e perde exit code mapping."

### Causa x Efeito

- Exit codes determinísticos **causa** necessidade de enum tipado.
- Enum tipado **causa** thiserror em vez de anyhow.
- Decisão consciente **fixa** conformidade real ao objetivo do PRINC-003.

---

## GAP-017 — pub use excessivos em src/lib.rs:120-131 (14+ símbolos públicos)

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/lib.rs:120-131`

### Problema

`pub use` em src/lib.rs reexporta 14+ símbolos como API pública. Alguns não precisam ser públicos:

```rust
pub use cache::{cache_path, default_ttl, invalidate_cache, read_cache, write_cache};  // linha 120
pub use cli::{Cli, FormatArg, LanguageArg};  // linha 121
pub use error::{AppError, AppResult, NoSubtitleReason};  // linha 122
pub use io::{...};  // linha 123
pub use parse::srt_to_text;  // linha 127
pub use parse::video_id::extract_video_id;  // linha 128
pub use provider::{Format, Provider, ProviderA, ProviderB, ProviderChain, SubtitleInfo};  // linha 129
pub use retry::{retry_with_backoff, CircuitBreaker};  // linha 130
pub use text::{normalize_nfc, normalize_nfc_bytes};  // linha 131
```

### Análise por símbolo

- `Cli, FormatArg, LanguageArg` (cli): consumidos por `src/main.rs` e por `src/bin/snapshot.rs` (testes). **Justificado**.
- `AppError, AppResult, NoSubtitleReason` (error): consumidos em todo o código. **Justificado**.
- `Format, Provider, ProviderA, ProviderB, ProviderChain, SubtitleInfo` (provider): consumidos por `src/commands/` e `src/bin/snapshot.rs`. **Justificado**.
- `retry_with_backoff, CircuitBreaker` (retry): consumidos por `src/commands/extract.rs`. **Justificado**.
- `cache::{cache_path, default_ttl, invalidate_cache, read_cache, write_cache}`: consumidos por `src/commands/extract.rs` e `src/commands/batch.rs`. **Justificado** se houver usuário externo.
- `parse::srt_to_text`: consumido em `src/lib.rs` por exemplo de doc. **Justificado**.
- `parse::video_id::extract_video_id`: usado em `src/main.rs` ou `src/commands/`. Verificar.
- `io::{...}`: precisa ver. Se for só para testes, deveria ser `pub(crate)`.
- `text::{normalize_nfc, normalize_nfc_bytes}`: consumido em `src/parse/mod.rs:6` (interno). **NÃO justificado publicamente** — mover para `pub(crate)`.

### Solução

1. Manter `pub use` para símbolos que têm consumidores externos legítimos (Cli, AppError, Provider, Retry).
2. Trocar para `pub(crate) use` para `text::normalize_nfc, normalize_nfc_bytes` e `io::*` se não houver consumidor externo.
3. Adicionar teste em `tests/exports.rs` que compile-test que a API pública atual é a desejada (e nada mais).

### Causa x Efeito

- `pub use` sem critério **causa** poluição de API pública.
- Falta de consumer check **permite** crescimento descontrolado.
- Critério explícito "tem consumer fora do crate" **fixa** o escopo.

---

## GAP-018 — pub mod em 11 módulos; nem todos precisam ser `pub`

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/lib.rs:78-116`

### Problema

```rust
pub mod cache;        // 78
pub mod cli;          // 81
pub mod commands;     // 84
pub mod crypto;       // 89
pub mod error;        // 92
pub mod io;           // 95
pub mod logging;      // 98
pub mod parse;        // 101
pub mod provider;     // 105
pub mod retry;        // 108
pub mod secret_endpoints;  // 113 — GAP-007
pub mod text;         // 116
```

### Análise

- `cache, cli, commands, crypto, error, parse, provider, retry, secret_endpoints, text` — todos `pub` (alguns deveriam ser `pub(crate)`).
- `io, logging` — `pub` mas provavelmente deveriam ser `pub(crate)` (são detalhe de impl, não API).
- `secret_endpoints` — GAP-007, claramente deveria ser `pub(crate)`.

### Solução

1. `pub mod io` → `pub(crate) mod io` se `io` só é usado internamente.
2. `pub mod logging` → `pub(crate) mod logging` (logging é detalhe de init).
3. `pub mod text` → `pub(crate) mod text` se `text` só é usado por `parse` (interno).
4. Manter `pub` para módulos que têm `pub use` justificável.

### Causa x Efeito

- `pub mod` default **causa** superfície pública inflada.
- Falta de gate de CI **permite** acúmulo.
- `pub(crate) mod` para detalhe **fixa** o escopo.

---

## GAP-019 — clippy.toml com 0 bytes (vazio) — perde oportunidades de lint customizado

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `clippy.toml`

### Problema

`clippy.toml` tem 0 bytes. Defaults do Clippy são usados. Não há lints customizados para:
- `disallowed_methods` para proibir `unwrap()`, `panic!()` em código de produção
- `cognitive_complexity` threshold
- `too_many_arguments` threshold
- `missing_docs_in_private_items`

### Solução

Preencher `clippy.toml` com:

```toml
disallowed-methods = [
    { path = "std::panic", reason = "use thiserror::Error and Result" },
    { path = "tokio::runtime::Runtime::new", reason = "use #[tokio::main]" },
]
cognitive-complexity-threshold = 30
too-many-arguments-threshold = 8
missing-docs-in-private-items = true
```

### Causa x Efeito

- clippy.toml vazio **causa** lints permissivos.
- Defaults **permitem** complexidade cognitiva alta.
- Lints customizados **fixam** qualidade consistente.

---

## GAP-020 — NFR-002 (RSS < 100 MB) nunca medido em CI

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

PRD NFR-002 declara que o sistema DEVE consumir RSS < 100 MB. Não há teste ou CI gate que meça isso.

### Solução

Adicionar `tests/integration/rss_size.rs` que parseia fixture RSS conhecida e valida que `peak_memory < 100 MB`. Em CI, usar `cargo test --test rss_size --release` com medição via `/usr/bin/time -v` ou `dhat-rs`.

### Causa x Efeito

- Sem teste de RSS **causa** regressões silenciosas.
- Teste com fixture **fixa** verificação.

---

## GAP-021 — NFR-003 (binário release < 20 MB) — VERIFICADO AGORA: 8,4 MB (OK)

- **Data de verificação:** 2026-06-13
- **Severidade:** INFO (PASS)
- **Status:** VERIFICADO
- **Detectado por:** Reauditoria 2026-06-13

### Medição

```
$ ls -la target/release/youtube-legend-cli
-rwxr-xr-x. 1 comandoaguiar comandoaguiar 8770768 jun 13 19:45 target/release/youtube-legend-cli
$ du -h target/release/youtube-legend-cli
8,4M target/release/youtube-legend-cli
```

### Veredicto

8,4 MB < 20 MB. **PASS**. Registrar métrica em `docs_prd/metrics/nfr-003.json` para histórico.

### Causa x Efeito

- Build release com `opt-level=3, lto=thin, strip=true, panic=abort` em Cargo.toml **causa** binário compacto.
- 8,4 MB **fixa** margem de 11,6 MB para crescimento futuro.

---

## GAP-022 — NFR-008 (cargo install sem deps) não testado em matriz multi-target

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

PRD NFR-008 declara que `cargo install youtube-legend-cli` deve funcionar sem dependências de sistema. CI matrix em `.github/workflows/` precisa ser verificada.

### Solução

Verificar `cargo install --path .` em 4 targets: linux-gnu, linux-musl, darwin, windows-msvc. Cada target deve completar sem `apt install` ou `brew install` adicional.

### Causa x Efeito

- Sem matriz multi-target **causa** surpresas em release.
- Teste em 4 targets **fixa** portabilidade.

---

## GAP-023 — EC-024 (heurísticas múltiplas para HTML drift) sem fallback

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

EC-024 declara: "Sistema DEVE ter heurísticas múltiplas para detectar drift de HTML". Atualmente há apenas os selectors CSS hardcoded em provider_a.rs e provider_b.rs.

### Solução

Adicionar heurística secundária: parse do JSON-LD no HTML (schema.org VideoObject) como fallback. Se selector primário falhar, tentar JSON-LD. Se JSON-LD falhar, tentar regex.

### Causa x Efeito

- 1 selector **causa** quebra silenciosa em drift de HTML.
- Heurística secundária **fixa** resiliência.

---

## GAP-024 — Cross-compile a targets darwin AUSENTE em CI

- **Data de identificação:** 2026-06-13
- **Severidade:** MÉDIA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

Cargo.toml targets:
```
targets = ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl", "x86_64-pc-windows-msvc"]
```

Faltam:
- `x86_64-apple-darwin` (Intel Mac)
- `aarch64-apple-darwin` (Apple Silicon)
- `aarch64-unknown-linux-gnu` (ARM Linux server)

### Solução

Adicionar targets ao CI matrix. Adicionar ao `[package.metadata.docs.rs] targets` também.

### Causa x Efeito

- Ausência de darwin **causa** release sem suporte Mac.
- CI matrix em 7 targets **fixa** cobertura.

---

## GAP-025 — Ausência de teste unitário para src/text.rs (normalize_nfc)

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** FALSO POSITIVO CONFIRMADO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

`src/text.rs` tem 1 doctest (passando) mas ZERO `#[cfg(test)]` módulo com testes unitários. Edge cases: texto vazio, BOM, combining marks, NFC vs NFD de strings idênticas visualmente.

### Solução

Adicionar `#[cfg(test)] mod tests` em `src/text.rs` com 5 casos: empty, ascii, combining_marks, bom_strip, nfd_to_nfc.

### Causa x Efeito

- Apenas doctest **causa** cobertura de edge cases faltando.
- 5 unit tests **fixam** regressões.

---

## GAP-026 — Ausência de teste para src/io.rs (read_url_from_stdin, read_urls_from_stdin)

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13

### Problema

`src/io.rs` lida com stdin, mas não foi verificado se tem testes. Skipping # comments, empty lines, UTF-8 BOM, malformed URLs.

### Solução

Verificar `src/io.rs` conteúdo. Se não tiver `#[cfg(test)]`, adicionar testes com `std::io::Cursor` simulando stdin.

### Causa x Efeito

- Sem teste de stdin **causa** bugs em shell pipelines.
- Teste com Cursor **fixa** contrato.

---

## GAP-027 — pub mod text + pub use text em src/lib.rs:116,131 — text é interno

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** CORRIGIDO 2026-06-13
- **Detectado por:** Reauditoria 2026-06-13
- **Arquivos:** `src/lib.rs:116,131`, `src/text.rs`

### Problema

`src/text.rs` é detalhe de implementação (NFC normalization usado por `src/parse/mod.rs:6`). Mas está exposto como `pub mod text` e `pub use text::{normalize_nfc, normalize_nfc_bytes}` na API pública.

### Solução

Mover para `pub(crate) mod text` e remover o `pub use text::...` em src/lib.rs:131. Consumir via `crate::text::normalize_nfc` internamente.

### Causa x Efeito

- `pub mod text` **causa** API pública inflada.
- `pub(crate) mod text` **fixa** o escopo interno.

---

## GAP-032 — `chrono` em `[build-dependencies]` SEM `build.rs`

- **Data de identificação:** 2026-06-13
- **Severidade:** BAIXA
- **Status:** FALSO POSITIVO CONFIRMADO 2026-06-14 (premissa do plano errada: build.rs existe desde 2026-06-07 e referencia chrono via `chrono::Utc::now().to_rfc3339()`; ambas as entradas em `[dependencies]` e `[build-dependencies]` sao necessarias)

### Problema declarado

O plano `velvety-crafting-wilkes.md` Seção 4 GAP-032 declarava: `chrono` em
`[build-dependencies]` (Cargo.toml:80) mas `build.rs` não existe. Solução
proposta: remover chrono de `[build-dependencies]`.

### Análise (verificação 2026-06-14)

Evidência objetiva refuta a premissa:

- `ls -la build.rs` → 632 bytes, mtime `2026-06-07 12:10:40 -0300` (existe desde
  a data de criação do projeto).
- `rg -n chrono build.rs` → match na linha 17: `chrono::Utc::now().to_rfc3339()`.

O `build.rs` SEMPRE EXISTIU e USA `chrono` em runtime de compilação. Remover
chrono de `[build-dependencies]` QUEBRARIA o build. A entrada em
`[build-dependencies]` é necessária, não orfã.

### Consequência se o gap fosse aplicado cegamente

- `cargo build` falharia com `error[E0433]: failed to resolve: use of
  unresolved module or unlinked crate \`chrono\`` no build script.
- Release pipeline quebrado.
- CI falhando em todos os 9 jobs.

### Recomendação

Não é gap técnico. Cancelar a "Solução" do plano (remover chrono de
`[build-dependencies]`). Manter ambas as entradas: `chrono` em `[dependencies]`
para uso em runtime de aplicação e em `[build-dependencies]` para uso em
build.rs.

Documentar como FALSO POSITIVO CONFIRMADO 2026-06-14 ao lado de GAP-009,
GAP-013, GAP-015, GAP-016, GAP-021, GAP-025.

### Causa x Efeito

- Plano Seção 4 GAP-032 **causa** premissa errada (build.rs ausente).
- `ls -la build.rs` + `rg -n chrono build.rs` **fixa** a verificação.
- Premissa refutada **causa** cancelamento da "Solução" original.
- build.rs desde 2026-06-07 **fixa** a necessidade de chrono em
  `[build-dependencies]`.

---

## Resumo Atualizado dos Gaps (2026-06-14 02:00 BRT pós-turno)

| ID | Severidade | Descrição | Status |
|----|------------|-----------|--------|
| GAP-007 | CRÍTICA | pub mod secret_endpoints + 11 pub const | **CORRIGIDO 2026-06-14** (pub → pub(crate), snapshot.rs usa #[path]) |
| GAP-008 | ALTA | NFR-005 offline+cache hit sem teste | **CORRIGIDO 2026-06-14** (tests/integration/offline_cache.rs:87-174) |
| GAP-009 | BAIXA | dedup batch | **CORRIGIDO** (src/commands/batch.rs:14,42-46) |
| GAP-010 | ALTA | NFR-007 robots.txt | **CORRIGIDO 2026-06-14** (src/provider/robots.rs:216 + provider_a.rs:160 + provider_b.rs:316) |
| GAP-011 | BAIXA | #[instrument] cobertura | **CORRIGIDO 2026-06-14** (14/14 instrumentações ativas, incl. ProviderA::fetch_subtitle e ProviderB::fetch_subtitle) |
| GAP-012 | MÉDIA | Wiremock tests providers | **CORRIGIDO 2026-06-14** (provider_{a,b}_wiremock.rs:5 testes cada) |
| GAP-013 | BAIXA | GAP-009 desatualizado | **CORRIGIDO** (drift documental) |
| GAP-014 | BAIXA | GAP-011 desatualizado | **CORRIGIDO** (drift documental) |
| GAP-015 | INFO | 6 Regex em provider_b | FALSO POSITIVO CONFIRMADO (regex em JS inline é correto) |
| GAP-016 | INFO | PRINC-003 (anyhow vs thiserror) | DECISÃO ARQUITETURAL (AppError tipado com thiserror) |
| GAP-017 | MÉDIA | 14+ pub use em src/lib.rs | **CORRIGIDO 2026-06-14** (reduzido para 2: cli, error — justificados) |
| GAP-018 | BAIXA | 11 pub mod em src/lib.rs | REDUZIDO: text → pub(crate); outros 10 com consumidores legítimos |
| GAP-019 | BAIXA | clippy.toml vazio | **CORRIGIDO 2026-06-14** (3 disallowed-methods + cognitive-complexity + too-many-arguments) |
| GAP-020 | BAIXA | NFR-002 RSS < 100 MB | **CORRIGIDO 2026-06-14** (tests/integration/rss.rs:MAX_RSS_KIB 100*1024) |
| GAP-021 | INFO | NFR-003 binário 8,4 MB | VERIFICADO PASS |
| GAP-022 | MÉDIA | NFR-008 sem matriz multi-target | **CORRIGIDO 2026-06-14** (.github/workflows/ci.yml cross-compile: 6 targets) |
| GAP-023 | BAIXA | EC-024 sem heurística secundária | **CORRIGIDO 2026-06-14** (provider_a.rs:253-354 JSON-LD VideoObject) |
| GAP-024 | MÉDIA | Cross-compile darwin AUSENTE | **CORRIGIDO 2026-06-14** (ci.yml:108-111 x86_64-apple-darwin + aarch64-apple-darwin com continue-on-error) |
| GAP-025 | BAIXA | text.rs sem unit tests | FALSO POSITIVO CONFIRMADO (7 #[test] em src/text.rs:28-75) |
| GAP-026 | BAIXA | io.rs sem testes stdin | **CORRIGIDO 2026-06-14** (6 #[test] em src/io.rs:138-201) |
| GAP-027 | BAIXA | pub mod text desnecessário | **CORRIGIDO 2026-06-14** (pub(crate) mod text em lib.rs:115) |

## Gaps Falsos Positivos Identificados (recorde canônico 2026-06-13)

Os 6 itens abaixo foram inicialmente listados como gaps ABERTO mas, após análise
criteriosa contra o código real e a Constitution, são FALSO POSITIVO, DECISÃO
CONSCIENTE, JÁ CORRIGIDO ou VERIFICADO PASS. Esta seção substitui a lista
provisória anterior e formaliza o veredito para a posteridade.

### GAP-009 — JÁ CORRIGIDO (drift documental)

- **Problema declarado:** EC-015 (dedup de URLs em modo batch) não implementado.
- **Análise:** `src/commands/batch.rs:14` (`use std::collections::HashSet;`) e
  `src/commands/batch.rs:42-45` JÁ implementam a deduplicação via `HashSet`.
- **Recomendação:** Não é gap. Status de GAP-009 em gaps.md:160 atualizado para
  `CORRIGIDO 2026-06-13`. GAP-013 (Fase 23) fecha o drift documental.

### GAP-013 — FECHAR (drift documental secundário)

- **Problema declarado:** GAP-009 desatualizado em gaps.md.
- **Análise:** Lacuna puramente documental — código em
  `src/commands/batch.rs:14,42-45` atende ao EC-015 do PRD.
- **Recomendação:** Não é gap técnico. Drift tratado em Fase 23.

### GAP-015 — FALSO POSITIVO (regex idiomática em JS inline)

- **Problema declarado:** 4 `Regex::new` em `src/provider/provider_b.rs` violariam
  PRINC-007 (HTML parsing via `scraper`).
- **Análise:** Os alvos são variáveis JavaScript inline dentro de
  `<script>var tutoken=...; var htoken=...;</script>`. Não é HTML estruturado
  — é texto JS. `scraper::Html::parse` sobre o conteúdo de `<script>` não ajuda
  a extrair `tutoken=`. Regex é a ferramenta idiomática para esse caso.
- **Recomendação:** Não é gap. Decisão consciente registrada aqui para
  evitar reauditoria futura.

### GAP-016 — DECISÃO ARQUITETURAL CONFIRMADA (thiserror com enum tipado)

- **Problema declarado:** PRINC-003 (anyhow::Result em binários) violado — código
  usa `AppResult<T>` via thiserror com enum tipado `AppError`.
- **Análise:** Enum tipado via thiserror habilita:
  - `exit_code()` determinístico por variant (mapping 2/3/4/5/6/7);
  - `Display` localizado em pt-BR por variant (PRINC-002);
  - Match exaustivo em testes.
  DuckDuckGo research confirma convenção: libraries usam thiserror, binários usam
  anyhow. O código atual é híbrido razoável: core de library usa thiserror;
  `main.rs` pode usar `From<AppError> for anyhow::Error` se necessário.
- **Recomendação:** Não é gap. Decisão consciente confirmada.

### GAP-021 — VERIFICADO PASS (binário release < 20 MB)

- **Problema declarado:** NFR-003 (binário release < 20 MB) não verificado.
- **Análise:** `ls -la target/release/youtube-legend-cli` retornou
  8.776.768 bytes = 8,4 MB. Margem de 11,6 MB contra o limite de 20 MB.
- **Recomendação:** Não é gap. Gate `cargo build --release` em
  `ci.yml:49-58` é a salvaguarda contínua.

### GAP-025 — FALSO POSITIVO (text.rs já tem 7 unit tests)

- **Problema declarado:** `src/text.rs` sem unit tests.
- **Análise:** Leitura direta de `src/text.rs:28-76` revela 7 funções `#[test]`:
  `ascii_unchanged`, `accented_nfc_canonical`, `nfd_input_gets_canonicalized`,
  `japanese_katakana_nfc`, `emoji_nfc_unchanged`, `empty_string_unchanged`,
  `bytes_passthrough_on_non_utf8`.
- **Recomendação:** Não é gap. Cobertura já satisfatória.

## Pendências de Auditoria Original (referência)

Gaps NÃO documentados nesta varredura que foram mencionados em outras auditorias mas ficaram fora do escopo:

- NFR-002 (RSS < 100 MB) — GAP-020
- NFR-008 (cargo install sem deps) não testado em matriz multi-target — GAP-022
- Constitution PRINC-009 (versão síncrona) — GAP-032 falso positivo
- EC-024 (heurísticas múltiplas para HTML drift) sem fallback — GAP-023
- Cross-compile a targets darwin — GAP-024
- 26 arquivos `.bak` — VERIFICADO: find src -name "*.bak*" retorna 0 hits
