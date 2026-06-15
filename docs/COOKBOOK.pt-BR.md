# COOKBOOK

> Receitas práticas para usar a CLI de legendas a partir de um shell, um runner de CI ou um pipeline Python.

Idiomas: [Inglês](docs/COOKBOOK.md) | [Português Brasileiro](docs/COOKBOOK.pt-BR.md)

## Nota de Latência

A camada de cache vive em `~/.cache/youtube-legend-cli/`. Em cache quente, o corpo é servido do disco em cerca de um milissegundo. Em cache frio, a latência é dominada pelo round-trip HTTP upstream somado ao throttling: `youtube-direct` fica em média 800 ms a 1,5 s de ponta a ponta em uma conexão residencial típica, `provider_a` fica em média 1,5 s a 3 s, e `provider_b` fica em média 2 s a 4 s por causa do round-trip de assinatura com AES-256-CBC mais PBKDF2. O throttle de uma requisição por segundo é por cadeia, não por provedor; então downloads consecutivos em cache frio pagam o custo do throttle a cada chamada.

## Referência de Valores Padrão

| Configuração | Padrão | Flag |
|---|---|---|
| Idioma | `en` | `--lang` |
| Formato | `txt` | `--format` |
| Timeout HTTP | 30 s | `--timeout` |
| Log verboso | desligado | `--verbose` |
| Suprimir logs não-erro | desligado | `--quiet` |
| Envelope JSON | desligado | `--json` |
| Batch pelo stdin | desligado | `--batch` |
| User-Agent | nome da crate | `--user-agent` |
| TTL do cache | 24 horas | `--cache-ttl` |
| Pular leitura de cache | desligado | `--no-cache` |
| Nível de log | `warn` | `--log-level` |
| Formato de log | `text` | `--log-format` |
| Cor | `auto` | `--color` |
| Barras de progresso | ativadas | `--no-progress` |
| Dry run (só cache) | desligado | `--dry-run` |
| Assumir sim nos prompts | desligado | `--yes` |
| Provedor | `auto` | `--provider` |
| Preferir trilha ASR | desligado | `--asr` |
| Desabilitar cadeia de fallback | desligado | `--no-fallback` |


## How To baixar legendas de um vídeo

PROBLEMA: Um usuário te dá uma única URL do YouTube e você precisa da transcrição em texto puro no disco.

SOLUÇÃO: Passe a URL na linha de comando, redirecione `stdout` para um arquivo. O corpo cai no arquivo; logs e progresso caem no terminal.

```bash
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > legenda.txt
```

VERIFICAÇÃO: O arquivo existe, contém a transcrição e o terminal mostra zero ou mais linhas de progresso apenas no `stderr`.

```bash
wc -l legenda.txt
head -n 3 legenda.txt
```


## How To baixar em batch a partir de uma lista

PROBLEMA: Você tem um arquivo com uma URL por linha e precisa de uma transcrição para cada vídeo.

SOLUÇÃO: Passe `--batch` e redirecione o arquivo para o `stdin`. Cada linha é processada em ordem; uma falha não fatal em uma linha não aborta o restante.

```bash
youtube-legend-cli --batch < urls.txt > transcricoes.txt 2> batch.log
```

VERIFICAÇÃO: `transcricoes.txt` contém todos os corpos bem-sucedidos concatenados na ordem de entrada, separados por uma linha de cabeçalho, e `batch.log` mostra o status por URL.

```bash
grep -c "^=== " transcricoes.txt
cat batch.log
```


## How To parsear o envelope JSON em Python

PROBLEMA: Um pipeline precisa dos campos estruturados (`video_id`, `language`, `byte_size`, `body`) sem escrever regex.

SOLUÇÃO: Use `--json` para fazer a CLI emitir um envelope JSON de uma linha, depois parseie com `json.loads`.

```python
import json
import subprocess

result = subprocess.run(
    ["youtube-legend-cli", "--json", "https://youtu.be/dQw4w9WgXcQ"],
    capture_output=True,
    text=True,
    check=False,
)
envelope = json.loads(result.stdout)
if envelope.get("error"):
    raise SystemExit(f"erro do provedor: {envelope['error']}")
print(envelope["body"])
```

VERIFICAÇÃO: O script imprime a transcrição no `stdout` e sai com código `0`. Se o upstream estiver indisponível, `envelope["error"]` é um objeto estruturado e o script sai com código diferente de zero.


## How To trocar de provedor para CI

PROBLEMA: Execuções de CI precisam de um provedor determinístico para evitar flakes quando um upstream está degradado.

SOLUÇÃO: Passe `--provider` para fixar um único provedor e `--no-fallback` para desabilitar a cadeia. A CLI sai com `69` se o provedor fixado falhar, em vez de tentar o próximo.

```bash
youtube-legend-cli --provider youtube-direct --no-fallback \
  "https://youtu.be/dQw4w9WgXcQ" > legenda.txt
```

VERIFICAÇÃO: O código de saída é `0` em sucesso, `69` em indisponibilidade do upstream, e nunca `0` vinda de um provedor diferente do que você fixou.


## How To sobrescrever o TTL do cache

PROBLEMA: Um processo batch de longa duração precisa de uma janela de cache maior para que downloads repetidos do mesmo vídeo sejam gratuitos.

SOLUÇÃO: Passe `--cache-ttl` em horas. O valor é um inteiro positivo; a camada de cache aplica em toda leitura.

```bash
youtube-legend-cli --cache-ttl 168 \
  "https://youtu.be/dQw4w9WgXcQ" > legenda.txt
```

VERIFICAÇÃO: Uma segunda invocação do mesmo comando no mesmo vídeo termina em menos de 10 ms e não produz tráfego de rede upstream.

```bash
time youtube-legend-cli --cache-ttl 168 "https://youtu.be/dQw4w9WgXcQ" > /dev/null
```


## How To lidar com HTTP 429 do upstream

PROBLEMA: Um provedor responde com HTTP 429 e um cabeçalho `Retry-After`. O pipeline precisa esperar e tentar de novo.

SOLUÇÃO: A CLI já honra `Retry-After` internamente via `retry::retry_with_backoff`. De fora, a única coisa a fazer é expor o erro estruturado e esperar.

```bash
output=$(youtube-legend-cli --json "https://youtu.be/VIDEO" 2>/dev/null)
if [ "$(echo "$output" | jq -r '.error.kind')" = "rate_limited" ]; then
  retry_after=$(echo "$output" | jq -r '.error.retry_after_secs // 60')
  echo "rate limited, dormindo ${retry_after}s" >&2
  sleep "$retry_after"
  youtube-legend-cli --json "https://youtu.be/VIDEO"
fi
```

VERIFICAÇÃO: O primeiro comando sai com código `69` e emite um envelope JSON com `error.kind = "rate_limited"`. O segundo comando (após o sleep) termina com sucesso e `error` igual a `null`.


## How To debugar com log verboso

PROBLEMA: Um download falha e você precisa ver a cadeia de provedores, tentativas de retry e timings HTTP.

SOLUÇÃO: Combine `--verbose` com `--log-level debug` para obter eventos de tracing no `stderr` e manter o corpo limpo no `stdout`.

```bash
youtube-legend-cli --verbose --log-level debug \
  "https://youtu.be/dQw4w9WgXcQ" > legenda.txt 2> trace.log
```

VERIFICAÇÃO: `trace.log` contém linhas `event = "retry"` com número da tentativa e provedor escolhido, além de códigos de status HTTP por requisição.

```bash
grep '"event":"retry"' trace.log
grep '"event":"http_response"' trace.log | tail
```


## How To integrar em um pipeline de CI/CD

PROBLEMA: Um job de CI precisa baixar legendas de uma lista fixa de vídeos e falhar o build se algum vídeo não tiver transcrição.

SOLUÇÃO: Combine `--json`, `--no-fallback` para garantir determinismo e um loop de shell que verifica o código de saída por URL.

```bash
#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r url; do
  if ! youtube-legend-cli --json --no-fallback --provider youtube-direct "$url" \
       > "out/$(echo "$url" | sed 's|.*/||;s|?.*||').json" 2> "logs/$(date +%s).log"; then
## How To forçar o provider YouTube direto

PROBLEMA: Provedores third-party não indexam o vídeo, mas ele
tem legendas públicas no YouTube.

SOLUÇÃO: Passe `--provider youtube-direct` para fixar o
provider nativo e pular a cadeia de fallback. A CLI então fala
com o endpoint público do YouTube e emite um SRT limpo.

```bash
youtube-legend-cli --provider youtube-direct \
  --language pt-BR \
  "https://youtu.be/<id>" > legenda.srt
```

VERIFICAÇÃO: O SRT tem cues de timing canônicos do YouTube,
sem watermark de provedor, e o campo `provider` do envelope
mostra `youtube-direct`.

```bash
head -n 3 legenda.srt
youtube-legend-cli --json "https://youtu.be/<id>" | jq -r .provider
```


## How To diagnosticar falha do player.js

PROBLEMA: O vídeo é protegido por signature e a etapa de
decipher falha. Você precisa de um diagnóstico estruturado antes
de tentar de novo.

SOLUÇÃO: Use o binário companheiro `youtube-direct-probe`. Ele
inspeciona o `base.js` em cache, roda o decipher em uma signature
sintética e imprime um relatório JSON.

```bash
youtube-direct-probe <video-id>
```

VERIFICAÇÃO: O probe imprime um objeto JSON por linha com
`signature_status`, `player_js_version`, `cache_hit` e um campo
opcional `decipher_error` em falha.

```bash
youtube-direct-probe <video-id> | jq -r '.signature_status'
```

    echo "falha de CI em $url" >&2
    exit 1
  fi
done < urls.txt
```

VERIFICAÇÃO: O job sai com código `0` quando cada URL produziu um envelope JSON, sai com o código da CLI (`64`/`65`/`66`/`69`/`70`) quando uma URL falhou, e o diretório `out/` contém um arquivo JSON por vídeo.
