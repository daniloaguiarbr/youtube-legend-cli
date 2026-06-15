# Política de Segurança

[English](SECURITY.md) | [Português Brasileiro](SECURITY.pt-BR.md)

## Versões suportadas

| Versão  | Suportada         |
|---------|-------------------|
| 0.2.6   | sim (atual)       |

## Reportando uma vulnerabilidade

Por favor, **não** abra uma issue pública no GitHub para problemas
sensíveis de segurança.

Envie um relatório privado ao mantenedor no endereço listado no
campo `authors` de `Cargo.toml`. Criptografe material sensível com a
chave PGP do mantenedor (solicite por e-mail).

Inclua:

- Descrição clara do problema e do cenário de ataque.
- Passos para reproduzir, incluindo a versão afetada.
- Comportamento esperado e comportamento observado.
- Quaisquer mitigações ou workarounds conhecidos.

Você deve receber um reconhecimento em até 72 horas. O mantenedor
coordenará o cronograma de divulgação com você.

## Modelo de ameaça

`youtube-legend-cli` é uma CLI single-user, não interativa, que:

- Lê uma URL do YouTube a partir de um argumento posicional ou stdin.
- Realiza requisições HTTPS para um ou dois provedores de legendas
  terceirizados.
- Escreve o corpo da legenda decodificada em stdout.
- Armazena a última resposta bem-sucedida no diretório de cache padrão
  do usuário, indexada por `(video_id, language, format)`, com TTL
  padrão de 24 horas.
- Não coleta, transmite nem persiste qualquer telemetria.

O módulo `secret_endpoints` é **gitignored** e jamais é publicado em
releases do crate. Ele carrega hosts, paths, cookies e user agents
dos provedores que são intencionalmente redacted dos consumidores
open-source. Se você encontrar algum desses valores exposto em
artefato público, por favor reporte imediatamente.
