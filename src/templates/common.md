## Comandos do dia a dia

| Comando | O que faz |
| --- | --- |
| `cppm run dev` | Compila em Debug e executa |
| `cppm watch` | Recompila e reinicia sempre que você salva um arquivo |
| `cppm run test` | Compila e roda os testes (CTest) |
| `cppm watch test` | Roda os testes de novo a cada alteração |
| `cppm run build` | Compila em Release |
| `cppm build --fresh` | Refaz dependências e o cache do CMake (use quando algo "travar") |
| `cppm clean --dry-run` / `cppm clean` | Mostra / remove artefatos de build, mantendo fontes e dependências |
| `cppm doctor` | Verifica compilador, CMake, Ninja, Conan e SDKs |
| `cppm ui` | Interface interativa: catálogo de bibliotecas, dependências e comandos |

Argumentos depois de `--` vão para o programa: `cppm run dev -- --port 9090`.

No VS Code, `Ctrl+Shift+B` compila. Use `Tasks: Run Task` -> `cppm: run` para executar
ou `cppm: watch` para desenvolver com reinício automático. Se o editor abriu a pasta
pai, rode `cppm vscode --workspace ..`.

## Quando algo der errado

1. `cppm doctor` — mostra qual ferramenta está faltando ou com versão errada.
2. `cppm build --fresh` — recria o cache do CMake e revalida as dependências.
3. Arquivo `.cpp` novo não compila? Ele precisa estar em `src/`; o CMake procura
   `src/*.cpp` automaticamente (inclusive subpastas).
4. Header não encontrado? Coloque-o em `include/` ou ao lado do `.cpp` e use aspas:
   `#include "meu_header.h"`.

## O que versionar no Git

Versione `cppm.toml`, `cppm.lock`, `CMakeLists.txt`, `src/`, `include/`, `tests/` e o que
mais você criar. A pasta `.cppm/` é local (builds e caches) e já está no `.gitignore`.

Documentação completa do cppm: https://github.com/GaussNoob/cppm/blob/main/docs/guide.md
