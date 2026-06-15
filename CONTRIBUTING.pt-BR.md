# Contribuindo com o youtube-legend-cli

[English](CONTRIBUTING.md) | [Português Brasileiro](CONTRIBUTING.pt-BR.md)

Obrigado pelo interesse no projeto. Este documento explica como
configurar um ambiente de desenvolvimento, executar a suíte de testes
e submeter uma mudança.

## Código de Conduta

Todos os contribuidores devem seguir o
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Reportando bugs

Abra uma issue no GitHub com uma reprodução mínima. Inclua a saída
de `youtube-legend-cli --version` e a linha de comando exata que
disparou o bug, com a URL redigida se necessário.

## Reportando vulnerabilidades de segurança

Veja [SECURITY.md](SECURITY.md). Não abra issues públicas para
problemas sensíveis de segurança.

## Ambiente de desenvolvimento

- Rust 1.88.0 ou mais recente (o CI roda stable, beta e a
  toolchain fixada).
- `cargo fmt`, `cargo clippy`, `cargo test`, `cargo bench` e
  `cargo doc` são as ferramentas obrigatórias. `mimalloc`,
  `criterion`, `wiremock`, `assert_cmd`, `predicates`,
  `serial_test` e `libc` são resolvidos automaticamente pela
  resolução de dependências padrão.
- A feature `headless` é opt-in:
  `cargo build --features headless`. A feature `headless` puxa
  `chromiumoxide` e `futures`, e requer uma instalação local de
  Chromium/Chrome em runtime.

## Fluxo de trabalho

1. Faça um fork do repositório e crie uma branch de tópico.
2. Faça sua mudança. Adicione ou atualize testes.
3. Execute os oito gates de qualidade antes de abrir um PR:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo build --release --all-features
   cargo test --lib --all-features
   cargo test --doc --all-features
   cargo bench --no-run
   cargo doc --no-deps --all-features -- -D warnings
   cargo deny check    # quando cargo-deny está no PATH
   cargo audit         # quando cargo-audit está no PATH
   ```

4. Abra um pull request contra `main`.

## Fluxo de Agent Teams

A release v0.2.6 foi entregue através da feature Agent Teams do
Claude Code. O playbook fica em
[`docs/agent-teams-workflow.md`](docs/agent-teams-workflow.md). As
regras de alto nível:

- Uma e apenas uma tarefa por arquivo. Duas tarefas editando o mesmo
  arquivo são mescladas antes do spawn.
- `Cargo.toml` é um arquivo serializado sob Agent Teams; veja
  [`docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md`](docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md)
  para a justificativa.
- Todas as mutações de arquivo passam pelo `atomwrite` para que o
  checksum BLAKE3 seja capturado por escrita e um state drift
  (exit 82) aborte a operação.
- A fase de validação executa os oito gates de qualidade acima. O
  relatório próprio de um teammate é informativo; os gates são a
  fonte da verdade.

## Estilo

- Rust edition 2021, MSRV 1.88.0 (declarado em `Cargo.toml`).
- Todos os itens públicos devem ter um `///` doc comment.
- Todos os blocos `unsafe` devem carregar uma linha `// SAFETY:`
  que explique o invariante sendo mantido.
- Mensagens de erro e impls de `Display` são escritos em inglês;
  esta é uma CLI técnica consumida por scripts e outras ferramentas.
- Formatação `cargo fmt` é canônica; não formate à mão.
- O crate é `#![deny(rustdoc::bare_urls)]` e
  `#![deny(rustdoc::invalid_html_tags)]`; evite URLs cruas em doc
  comments e prefira a forma `[texto](url)`.

## Mensagens de commit

- Modo imperativo, assunto de uma linha com menos de 72 caracteres,
  corpo opcional quebrado em 72 colunas.
- Referencie a issue ou PR relevante quando aplicável.
- Não inclua trailers `Co-authored-by`.

## Licença

Ao contribuir você concorda que sua contribuição é duplamente
licenciada sob os termos de [LICENSE](LICENSE) (MIT ou Apache-2.0,
a critério do mantenedor).
