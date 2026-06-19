# Gaps Conhecidos


## GAP-2026-001 — Ausência de Provider Headless para Bypass de Anti-Bot


### Problema
- A CLI usa apenas reqwest puro para acessar o YouTube e provedores terceiros
- O YouTube retorna HTTP 400 Bad Request para requisições deste IP datacenter
- O provedor Provider A depende de downsub.com que requer JavaScript para gerar tokens
- A CLI não executa JavaScript nem possui browser headless integrado
- O User-Agent default declara literalmente youtube-legend-cli/0.3.0 ao YouTube
- A CLI falha ao baixar legendas de vídeos públicos que downsub.com aceita no browser


### Consequências
- Usuários não conseguem baixar legendas de forma automatizada pela CLI
- O comando youtube-legend-cli retorna EX_SOFTWARE 70 ou EX_NOINPUT 66 sistematicamente
- Cada execução da CLI gasta timeout de 30 segundos antes de falhar
- A reputação da ferramenta cai entre usuários técnicos que comparam com sites web
- Operadores precisam recorrer a sites web quebrando o fluxo agent-first
- LLMs que orquestram a CLI recebem falhas estruturais sem caminho de recuperação
- O ecossistema Rust perde um caso de uso real para ferramentas agent-first


### Causa Raiz
- A CLI usa reqwest com feature rustls que produz TLS fingerprint de datacenter
- O YouTube detecta TLS fingerprint não humanoide via JA3 e bloqueia requisição
- O User-Agent youtube-legend-cli/0.3.0 é assinatura óbvia de bot para servidores
- A CLI não envia headers Sec-Fetch-Dest Sec-Fetch-Mode Sec-Fetch-Site Sec-Ch-Ua
- O downsub.com executa JavaScript para gerar token dinâmico /eyJ... por requisição
- O token dinâmico requer resolução de challenge Cloudflare via JavaScript no browser
- A feature headless existe apenas como comentário em src/lib.rs sem implementação
- O Provider A em src/provider/provider_a.rs depende de scraping HTML estático
- O reqwest sem browser engine não consegue extrair tokens gerados via JavaScript
- O cookie cf_clearance do Cloudflare não é persistido entre invocações da CLI
- O retry em src/retry.rs só trata AppError::RateLimited e ignora HTTP 400 anti-bot


### Solução
- Ativar feature headless do projeto já planejada no código fonte
- Adicionar dependência chromiumoxide com features fetcher rustls zip8
- Criar src/provider/provider_headless.rs com Provider trait implementation
- Spawnar Chromium real via DevTools Protocol controlado pela CLI
- Navegar até downsub.com/?url=ENCODED como browser humano faria
- Aguardar challenge Cloudflare resolver automaticamente no browser
- Extrair link da legenda via page evaluate após token /eyJ... aparecer no DOM
- Baixar SRT final via reqwest usando cookies extraídos do browser
- Fechar browser de forma graciosa após download ou em caso de erro
- Implementar fallback gracioso para provider youtube-direct quando headless indisponível
- Detectar ausência de Chrome Chromium google-chrome no sistema com mensagem clara
- Reusar BrowserFetcher para download automático de Chromium quando ausente


### Benefícios da Solução
- A CLI replica exatamente o comportamento de um humano navegando no browser
- O bypass de Cloudflare funciona porque o challenge é resolvido via JavaScript real
- O YouTube aceita requisições porque o browser tem TLS fingerprint humanoide
- O cookie cf_clearance é gerenciado automaticamente pelo browser entre requisições
- Não há dependência de IP residencial nem de proxies pagos
- O custo da solução é zero pois usa apenas Chromium instalado ou baixado
- A solução é resiliente a mudanças no YouTube e Cloudflare pois é browser real
- A feature headless segue as rules rust de subprocessos externos com timeout
- A solução escala naturalmente para outros provedores que requerem JavaScript
- A documentação do projeto passa a refletir a feature já mencionada no código


### Como Solucionar
- Adicionar chromiumoxide 0.7 como dependência opcional em Cargo.toml
- Criar feature flag headless que habilita chromiumoxide e tokio-stream
- Adicionar BrowserLauncher que detecta Chrome Chromium google-chrome no PATH
- Implementar ProviderHeadless com builder pattern consistente com Provider A
- Configurar BrowserConfig com window dimensions headless true e disable gpu
- Adicionar flag --headless na CLI para acionar ProviderHeadless na cadeia
- Implementar timeout de 60 segundos para challenge Cloudflare resolver
- Capturar cookies via page get_cookies após navigation bem-sucedida
- Construir reqwest::Client efêmero com cookies do browser para download final
- Adicionar testes de integração com wiremock para ProviderHeadless
- Documentar a feature headless em docs/AGENTS.pt-BR.md
- Atualizar README com exemplo de uso youtube-legend-cli --headless URL
- Validar compilação com cargo build --features headless em CI matrix
- Validar testes com cargo test --features headless em Linux macOS e Windows


## GAP-2026-002 — provider_headless ignora YT_LEGEND_NO_NETWORK


### Problema
- A função ProviderHeadless::fetch_subtitle spawna Chromium e acessa a rede sem consultar a env var YT_LEGEND_NO_NETWORK
- A regra do projeto diz que YT_LEGEND_NO_NETWORK desabilita todo o tráfego de rede em modo offline
- CI e auditorias offline terminam com timeout em vez de sair limpo
- A documentação do env var em CHANGELOG.md:46 lista o var mas provider_headless não honra


### Consequências
- Pipelines agent-first com rede bloqueada falham com EX_SOFTWARE 70 após 30s
- Aderência parcial ao contrato de offline-safe quebra expectativa do operador
- Logs de CI ficam poluídos com stack traces de timeout em vez de indicar skip limpo
- Usuários não conseguem reproduzir bugs sem rede sem isolar o provider manualmente


### Causa Raiz
- src/provider/provider_headless.rs::fetch_subtitle não consulta std::env::var YT_LEGEND_NO_NETWORK
- A feature headless foi adicionada na v0.2.6 sem essa guarda
- Os outros providers (youtube-direct provider-a provider-b) já honram a env via outras camadas
- O retry em src/retry.rs não cobre o caso porque o spawn do browser antecede qualquer retry


### Solução
- Adicionar guarda no topo de fetch_subtitle que retorna AppError::ProviderUnavailable quando YT_LEGEND_NO_NETWORK está setada
- Documentar a regra no doc comment da struct ProviderHeadless
- Adicionar teste de integração yt_legend_no_network_env_blocks_fetch_subtitle em tests/integration/provider_headless_wiremock.rs
- Manter o valor da env var irrelevante apenas a presença importa


### Benefícios da Solução
- Modo offline fica verdadeiramente offline sem spawnar browser
- CI com rede bloqueada sai com EX_UNAVAILABLE 69 em vez de timeout 70
- Aderência ao contrato de env vars YT_LEGEND_* fica uniforme entre providers
- Logs de auditoria ficam limpos sem stack traces espúrios


### Como Solucionar
- Adicionar if std::env::var YT_LEGEND_NO_NETWORK.is_ok return Err AppError::ProviderUnavailable no início de fetch_subtitle
- Atualizar doc comment da struct ProviderHeadless para mencionar a env var
- Criar teste de integração provider_headless_wiremock com serial_test para manipular a env var com segurança
- Validar com cargo test --features headless em CI matrix


## GAP-2026-003 — duckduckgo-search-cli bloqueia datacenter IP


### Problema
- O binário duckduckgo-search-cli retorna 0 resultados consistentemente neste ambiente de desenvolvimento
- O IP do datacenter onde a sessão roda está na blacklist do DuckDuckGo
- A cascata anti-bot v0.6.4+ já rotacionou 5 identidades sem sucesso
- O endpoint html é bloqueado e o fallback lite não está habilitado


### Consequências
- Pesquisa técnica via duckduckgo-search-cli retorna 0 para qualquer query
- Plano de implementação depende primariamente de context7 e análise de código local
- A skill duckduckgo-search-cli não cumpre sua missão neste ambiente
- Workflows que assumem pesquisa web como fonte primária ficam incompletos


### Causa Raiz
- Datacenter IP está em blacklist do DuckDuckGo Cloudflare challenge
- Endpoint html retorna HTTP 202 anomaly modal que a cascata trata como interstitial
- Endpoint lite não está habilitado por padrão requer --allow-lite-fallback
- Não há proxy configurado para o ambiente atual
- A regra v0.6.4+ de não fazer retry em shell foi respeitada


### Solução
- Reconhecer a limitação no plano de release da v0.3.1 e depender primariamente de context7 para pesquisa
- Documentar o GAP-2026-003 em gaps.md para tracking
- Configurar proxy socks5 via --proxy para próximos lançamentos se necessário
- Habilitar --allow-lite-fallback como opt-in no ambiente de CI
- Usar context7 library para descobrir bibliotecas e context7 docs para validar APIs


### Benefícios da Solução
- Plano de release v0.3.1 é executável mesmo com pesquisa web bloqueada
- Conhecimento do gap permite mitigação em sessões futuras
- Dependência primária de context7 garante cobertura de documentação oficial
- Documentação do gap ajuda outros agentes a evitar o mesmo bloqueio


### Como Solucionar
- Adicionar GAP-2026-003 em gaps.md com causa raiz e mitigações
- Configurar --proxy socks5://127.0.0.1:9050 em ambiente com Tor disponível
- Habilitar --allow-lite-fallback em pipelines CI que precisam de resultados web
- Trocar para --endpoint lite após exit 3 confirmado para degradação aceitável
- Esperar 300+ segundos antes de retentar quando cascata nível 4 for atingido
