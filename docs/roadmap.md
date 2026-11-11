# Roadmap: paridade com o pnpm

O objetivo do cppm é dar a projetos C++ o fluxo que o pnpm dá a projetos
JavaScript: um manifesto pequeno, um lockfile confiável, um armazenamento
compartilhado e comandos rápidos. Esta página compara os dois e ordena o que
falta. As prioridades consideram o que muda no dia a dia de um projeto C++, não
apenas a lista de comandos do pnpm.

## Onde o cppm está

| pnpm | cppm | Estado |
| --- | --- | --- |
| `pnpm add`, `remove`, `install` | `cppm add`, `remove`, `install` | Pronto |
| `pnpm add -D` | `cppm add -D`, `[dev-dependencies]` | Pronto |
| `pnpm install --frozen-lockfile` | `--frozen-lockfile`, `cppm ci` | Pronto |
| `pnpm list`, `why` | `cppm list`, `why` | Pronto |
| `pnpm outdated`, `update` | `cppm outdated`, `upgrade` | Pronto (versões exatas) |
| `pnpm licenses list` | `cppm licenses` | Pronto |
| `pnpm run`, `exec`, `test` | `cppm run`, `exec`, `test`, `watch` | Pronto |
| Store endereçado por conteúdo | `cppm store` com hard links BLAKE3 | Pronto |
| `pnpm store prune`, `status`, `path` | `cppm store prune`, `status`, `path` | Pronto |
| Instalação sem trabalho repetido | Build sem alterações não inicia o Conan | Pronto |
| Faixas SemVer no manifesto | Versões exatas | Falta |
| Workspaces, `--filter`, `-r` | Um projeto por manifesto | Falta |
| `pnpm.overrides` | Somente por perfil Conan | Falta |
| `pnpm patch` | — | Falta |
| `pnpm fetch` | — | Falta |
| `pnpm link` | Conan editable, manual | Falta |
| `pnpm dlx` | — | Falta |
| `pnpm audit` | — | Falta |
| `pnpm completion` | — | Falta |

`cppm update` continua sendo a atualização do próprio cppm (o `pnpm
self-update`); por isso o comando de dependências se chama `upgrade`, com o
alias `up`.

## Próximos passos, em ordem

### 1. Faixas de versão no manifesto

Hoje `fmt = "11.0.2"` é exato e `cppm upgrade` aplica a regra do circunflexo no
momento da atualização. O passo seguinte é gravar a intenção:

```toml
[dependencies]
fmt = "^11.0.2"     # qualquer 11.x a partir de 11.0.2
zlib = "~1.3.1"     # apenas correções 1.3.x
boost = "1.91.0"    # exato, como hoje
```

O Conan já resolve faixas (`fmt/[>=11.0.2 <12]`), e o `cppm.lock` já fixa a
revisão escolhida. O trabalho é traduzir `^`/`~` para a sintaxe do Conan no
`conanfile` gerado, manter a versão exata no lockfile e fazer `outdated` usar a
faixa declarada como `Wanted`. `conan::versions` já tem a ordenação e a regra
de compatibilidade.

### 2. Workspaces

Um `cppm-workspace.toml` na raiz listando os membros, com um único lockfile e
uma única resolução para todos:

```toml
members = ["apps/*", "libs/*"]
```

- `cppm install` na raiz resolve o grafo unido; cada membro recebe sua
  integração CMake.
- `fmt = { workspace = true }` herda a versão de um catálogo na raiz, como os
  `catalog:` do pnpm, evitando versões divergentes entre membros.
- `cppm -r build` e `cppm --filter app... test` executam por membro, em ordem
  topológica.
- Um membro pode depender de outro (`core = { path = "../libs/core" }`), o que
  exige o item 3.

É a maior mudança: `Project` passa a ter um contexto de workspace, e o lock do
projeto e o registro do armazenamento passam a ser por raiz.

### 3. Bibliotecas, não só aplicações

`package.type = "library"` com um template `cppm init --template lib` que gera
`add_library`, headers públicos, `install()` e um `conanfile` exportável. Com
isso `cppm pack` cria o pacote Conan local, `cppm publish` envia a um remoto e
dependências `path =`/`git =` passam a funcionar. É o que transforma o cppm de
consumidor em gerenciador de pacotes completo.

### 4. Overrides e patches

```toml
[overrides]
zlib = "1.3.1"            # força a versão em todo o grafo

[patches]
"fmt@11.0.2" = "patches/fmt-locale.patch"
```

Overrides mapeiam para `[replace_requires]` do Conan em um perfil gerado.
Patches exigem exportar uma receita derivada para o armazenamento, com a
identidade do patch no `cppm.lock`. `cppm patch fmt` e `cppm patch-commit`
seguiriam o fluxo do pnpm: extrair os fontes, editar, gravar o diff.

### 5. Opções de pacotes no manifesto

Hoje opções como `opencv/*:with_ffmpeg=True` exigem um perfil Conan. Uma seção
no manifesto deixa a escolha versionada junto do projeto:

```toml
[dependencies]
opencv = { version = "4.14.0", options = { with_ffmpeg = true } }
```

As opções entram no `default_options` do `conanfile` gerado, que já existe para
os padrões de OpenCV e Qt.

### 6. `cppm fetch` e cache de CI

`cppm fetch` baixa para o armazenamento tudo o que o `cppm.lock` fixa, sem
precisar dos fontes do projeto: em uma imagem Docker, a camada de dependências
só é refeita quando o lockfile muda. Junto disso, documentar a chave de cache
de CI recomendada (`cppm.lock` + perfil) e um `cppm store export/import` para
transportar binários entre runners.

### 7. Dependências Conan em WebAssembly

Um perfil Emscripten gerado pelo cppm (host `emscripten`/`wasm`, build nativo)
remove a restrição atual do template WASM. O armazenamento já separa binários
por perfil; falta distinguir os contextos host e build, hoje iguais.

### 8. Conveniências

- `cppm completions bash|zsh|fish|powershell`.
- `cppm link ../minha-lib` sobre `conan editable`.
- `cppm dlx ferramenta` para executar um binário de um pacote sem adicioná-lo.
- `cppm audit` consultando uma base de vulnerabilidades (OSV) pelas versões do
  lockfile.
- `cppm ui` mostrando `[dev-dependencies]`, `outdated` e a árvore de `list`.
- Hooks `pre<script>`/`post<script>` opcionais em `cppm run`.

## Decisões que valem manter

- **Conan como resolvedor.** Compatibilidade binária de C++ (compilador,
  libcxx, opções) é o problema difícil; o Conan já o resolve. O cppm continua
  sendo a camada de fluxo de trabalho.
- **Reuso provado, não presumido.** Um atalho só existe quando há uma
  verificação que o invalida. A chave rápida segue essa regra: o que não pode
  ser enumerado (perfis com templates) volta para o Conan.
- **Falha não altera o projeto.** `add`, `remove` e `upgrade` instalam antes de
  gravar manifesto e lockfile.
