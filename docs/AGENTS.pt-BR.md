# AGENTS

> Um fetch de legendas no estilo Unix nativo que dá a agentes controle total do fluxo de bytes.

Idiomas: [Inglês](docs/AGENTS.md) | [Português Brasileiro](docs/AGENTS.pt-BR.md)

## Por que

- Você é um agente, não um humano. A CLI entrega uma URL na entrada, uma legenda na saída, em `stdin` e `stdout` puros. Sem prompts, sem TUI, sem daemon para monitorar.
- Você já fala JSON. Passe `--json` e a CLI entrega um envelope tipado com `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url` e o corpo. Sem parsear strings, sem regex frágil sobre texto livre.
- Você já fala códigos de saída. A CLI retorna números BSD `sysexits.h` para que pipelines POSIX, scripts com `set -e` e seus manipuladores de erro ramifiquem por categoria sem mapeamento customizado.

## Economia

- Um download de legenda único é cerca de 60 por cento menor em tokens do que raspar a página HTML do player e extrair as legendas manualmente. A cadeia de provedores retorna o payload do timedtext diretamente, no idioma pedido, no formato pedido.
- O cache local em `~/.cache/youtube-legend-cli/` é indexado por `(video_id, language, format)`. Pedidos repetidos do mesmo vídeo na mesma sessão são servidos do disco em microssegundos.
- O envelope JSON tem uma linha. Seu parser lê uma vez, o contexto do LLM guarda o corpo, o prompt cabe na janela.

## Soberania

- O binário é um único artefato estático em Rust. Sem dependências de runtime, sem contêiner, sem nuvem, sem daemon, sem processo em segundo plano. Coloque no host e rode.
- Zero telemetria. A CLI nunca liga para casa, nunca envia analytics, nunca verifica atualizações. O único tráfego de saída é a requisição HTTP ao provedor escolhido, escopada ao vídeo pedido.
- O módulo `secret_endpoints` é `pub(crate)` e está no `.gitignore`. Hostnames upstream, caminhos de cookies e tokens de assinatura nunca entram no rustdoc publicado nem na baseline da `public-api`.
- `SIGINT` e `SIGTERM` são cooperativos. O primeiro sinal cancela o trabalho em curso no próximo ponto de `await` e sai com código `130`. O segundo sinal força saída imediata do processo.

## Agentes Compatíveis

- Claude Code — envie uma URL via `stdin`, capture o envelope JSON via `stdout`, ramifique pelo código de saída. Funciona em uma ferramenta `Bash`, em um job `cron`, em um hook.
- Aider — chame a CLI a partir de um bloco de comando shell, parseie a saída `--json`, alimente o corpo de volta na próxima edição.
- Codex CLI — dispare o binário como subprocesso, leia `stdout`, trate `stderr` apenas como diagnóstico.
- Cline — use a CLI como ferramenta, passe uma URL, capture o resultado, nunca toque a página HTML do player.
- Qualquer agente LLM com ferramenta `bash`. A interface é Unix puro.

## Arquitetura em um Relance

Uma única struct `Cli` derivada de `clap` captura as 17 flags. `commands::run` despacha para `extract::run` quando é uma URL única ou para `batch::run` para listas vindas do `stdin`. A cadeia de provedores percorre `youtube-direct`, depois `provider_a`, depois `provider_b` (e `provider_headless` quando a feature `headless` está habilitada), limitada a uma requisição por segundo, envolta em `retry::retry_with_backoff` com três tentativas em 1 s, 2 s, 4 s. A camada de cache grava toda busca bem-sucedida em disco. A camada de saída escreve texto puro, SRT ou o envelope JSON no `stdout`; logs e progresso vão para o `stderr`.


## Flags da CLI

### OBRIGATÓRIO

- Use `--json` sempre que um consumidor downstream precisar parsear a saída. Texto puro é para humanos e pipes que não ligam para estrutura.
- Passe `--lang` com uma tag BCP 47 (`pt-BR`, `en-US`, `pt_BR.UTF-8`) quando precisar de idioma específico. O padrão `en` é um chute.
- Defina `--timeout` em segundos para fluxos limitados por rede. O padrão de 30 segundos serve para uso interativo; pipelines longos devem aumentar.

### PROIBIDO

- Não hardcode o hostname do provedor no código do agente. A cadeia de provedores é o contrato público; os hostnames estão no `.gitignore` e podem mudar sem aviso.
- Não parseie `stderr`. Logs são legíveis por humanos e podem incluir spans de tracing; os dados estruturados vivem no `stdout` quando `--json` está ativo.
- Não use o provedor `headless` sem a feature flag `headless`. O binário se recusa a invocar Chromium sem o portão da feature em build time.

### Padrão Correto

```bash
youtube-legend-cli --json "https://youtu.be/dQw4w9WgXcQ" \
  | jq -r '.body'
```

Descubra todas as flags com `youtube-legend-cli --help`. A tabela completa vive no `README.md` do projeto.


## Envelope JSON

### OBRIGATÓRIO

- Parseie `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url` como campos tipados. Não os extraia com regex.
- Ramifique pelo campo `error` quando ele não for nulo. O envelope é a fonte da verdade para modos de falha.

### PROIBIDO

- Não ignore o campo `error`. Todo envelope sem sucesso carrega uma falha estruturada com `kind` e mensagem humana.
- Não assuma que o corpo é uma string. É string UTF-8 quando `--format txt` está ativo e payload SRT byte a byte quando `--format srt` está ativo.

### Padrão Correto

```json
{
  "provider": "youtube-direct",
  "video_id": "dQw4w9WgXcQ",
  "language": "en",
  "format": "txt",
  "byte_size": 1452,
  "source_url": "https://www.youtube.com/api/timedtext...",
  "body": "...",
  "error": null
}
```


## Cadeia de Provedores

### OBRIGATÓRIO

- Deixe a cadeia fazer fallback automático. A política `auto` tenta `youtube-direct` primeiro, depois `provider_a`, depois `provider_b` (e `provider_headless` se o binário foi compilado com a feature).
- Honre `--asr` quando o requisitante quer a trilha auto-gerada mesmo quando existe uma trilha manual.

### PROIBIDO

- Não fixe um único provedor em CI de produção. O ponto da cadeia é degradação graciosa quando um upstream está degradado.
- Não combine `--asr` com `provider_a` ou `provider_b`. Os provedores terceiros não expõem seleção manual versus ASR; a CLI rejeita a combinação com código de saída `64`.

### Padrão Correto

```bash
# Produção: deixa a cadeia fazer fallback automático
youtube-legend-cli --provider auto "https://youtu.be/VIDEO"

# Debug: fixa um provedor e desabilita fallback
youtube-legend-cli --provider youtube-direct --no-fallback "https://youtu.be/VIDEO"
```


## Códigos de Saída

### OBRIGATÓRIO

- Ramifique pela categoria do BSD `sysexits.h`. `0` é sucesso, `64` é erro de uso, `65` é erro de dados, `66` é sem entrada, `69` é indisponível, `70` é erro de software, `78` é erro de configuração, `130` é sinal.
- Use `AppError::exit_code()` quando consumir a API Rust diretamente. O mapeamento é a mesma tabela.

### PROIBIDO

- Não hardcode inteiros brutos em scripts de CI. Mapeie por nome de categoria no seu dispatcher de shell.
- Não trate `69` como erro fatal. Significa que o upstream estava indisponível; tente de novo com backoff e provedor diferente.

### Padrão Correto

```bash
case "$(youtube-legend-cli --json ...; echo $?)" in
  0)   tratar_sucesso ;;
  64|65|78) tratar_erro_usuario ;;
  66)  tratar_sem_legenda ;;
  69)  tratar_upstream_indisponivel ;;
  70)  tratar_erro_interno ;;
  130) tratar_sinal ;;
  *)   tratar_desconhecido ;;
esac
```


## Cache

### OBRIGATÓRIO

- Use o TTL padrão de 24 horas. A camada de cache em `~/.cache/youtube-legend-cli/` é indexada por `(video_id, language, format)` e é segura para compartilhar entre execuções.
- Use `--no-cache` para leituras pontuais que precisam refletir o estado atual do upstream, não o snapshot em cache.

### PROIBIDO

- Não redirecione o cache para `/tmp`. O diretório de cache é criado e gerenciado pela crate `directories`; contorná-lo perde o benefício entre execuções.
- Não edite manualmente os arquivos de cache. O formato é interno e a próxima execução sobrescreve entradas inconsistentes.

### Padrão Correto

```bash
# Sobrescreve o TTL para um batch de longa duração
youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"

# Força leitura fresca, ignora cache
youtube-legend-cli --no-cache "https://youtu.be/VIDEO"
```


## Retry e Rate Limiting

### OBRIGATÓRIO

- Honre o cabeçalho `Retry-After` em ambas as formas, delta-seconds e RFC 2822 HTTP-date. A CLI já faz isso em `retry::retry_with_backoff`; o fallback é 60 segundos, com teto de 300.
- Trate `AppError::RateLimited` como transitório. O provedor se recupera; a cadeia tenta de novo.

### PROIBIDO

- Não adicione um loop de retry customizado no código do agente. A CLI já tem backoff próprio e circuit breaker; aninhar retries causa stampedes.
- Não fixe `--timeout` abaixo de 5 segundos. A primeira requisição é limitada a uma por segundo; um timeout apertado dispara falhas espúrias.

### Padrão Correto

```bash
# A CLI trata Retry-After internamente
youtube-legend-cli "https://youtu.be/VIDEO"
## Capacidades do Provider YouTube Direct (v0.3.0)

Agentes que consomem a CLI podem agora:

- Passar `--provider youtube-direct` para forçar o provider nativo.
- Confiar em legendas auto-geradas (ASR) sem fallback para third-party.
- Receber SRT canônico do YouTube (Srv3/Json3 convertidos localmente).
- Diagnosticar com `youtube-direct-probe <video-id>` (binário de probe).

## Comportamento de Erros (v0.3.0)

Novas variantes em `AppError`:

- `SignatureDecipherFailed(String)`: exit 70 (`EX_SOFTWARE`).
- `PlayerResponseMissing(String)`: exit 70.
- `CaptionTrackNotFound`: exit 70.
- `TimedtextUpstreamError(String)`: exit 70.


# Inspecione o Retry-After parseado ao consumir a API Rust
match err {
    AppError::RateLimited { retry_after_secs } => sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))),
    _ => return Err(err),
}
```


## Contratos de Stream

### OBRIGATÓRIO

- Trate `stdout` como o corpo da legenda, ou o envelope JSON quando `--json` está ativo. O contrato é exclusivo.
- Trate `stderr` como logs, progresso e mensagens de erro humanas. Capture para debug; nunca faça parse.

### PROIBIDO

- Não escreva seus próprios logs no `stdout`. O fluxo de bytes a jusante é a legenda; qualquer dado que não seja legenda corrompe a saída.
- Não redirecione `stderr` para `/dev/null` em CI. Você perderá o motivo da falha quando o código de saída for diferente de zero.

### Padrão Correto

```bash
# Captura o corpo e os logs separadamente
youtube-legend-cli "https://youtu.be/VIDEO" > subtitle.txt 2> run.log
```


## Tratamento de Erros

### OBRIGATÓRIO

- Mapeie `AppError` para uma categoria. O enum é `#[non_exhaustive]`; trate cada variante como categoria, não caso específico.
- Use o helper `reason()` para extrair o `NoSubtitleReason` interno quando o erro for `NoSubtitle`. O ramo padrão retorna `NotPublished`.

### PROIBIDO

- Não use `panic!` em `AppError`. A API da biblioteca é total; toda falha tem variante tipada e mensagem legível.
- Não transforme o erro em string para casar por substring. As variantes tipadas são o contrato; substrings são instáveis.

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

- Defina `YT_LOG_LEVEL` como um entre `error`, `warn`, `info`, `debug`, `trace` para sobrescrever `--log-level` em runtime. A CLI honra a env var acima do valor da CLI.
- Defina `YT_LOG_FORMAT=json` em produção para logs parseáveis por máquina. A CLI escreve apenas no `stderr`.

### PROIBIDO

- Não leia o ambiente diretamente do código do agente. A CLI consome `YT_LOG_LEVEL` e `YT_LOG_FORMAT`; deixe o binário fazer isso.
- Não defina `RUST_LOG` esperando que ela vença. A CLI usa um `EnvFilter` que prefere `YT_LOG_LEVEL` sobre `RUST_LOG`.

### Padrão Correto

```bash
# Produção: logs JSON estruturados em nível info
export YT_LOG_LEVEL=info
export YT_LOG_FORMAT=json
youtube-legend-cli "https://youtu.be/VIDEO"
```
