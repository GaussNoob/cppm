# {name}

Aplicação C++ de console criada com `cppm init`. Para começar:

```sh
cppm run dev      # compila e executa
cppm watch        # recompila e executa a cada vez que você salva
```

## Estrutura

```text
{name}/
  cppm.toml        # nome, padrão C++, dependências e scripts
  cppm.lock        # versões exatas instaladas (gerado)
  CMakeLists.txt   # build; pode ser editado à vontade
  src/main.cpp     # ponto de entrada
  .cppm/           # builds e caches locais (fora do Git)
```

## Próximos passos

### 1. Dividir o código em arquivos

Todo `.cpp` dentro de `src/` (inclusive subpastas) entra no build sozinho.
Headers podem ficar em `include/`, que já está no caminho de includes.

```cpp
// include/calculadora.h
#pragma once
int somar(int a, int b);
```

```cpp
// src/calculadora.cpp
#include "calculadora.h"
int somar(int a, int b) { return a + b; }
```

```cpp
// src/main.cpp
#include <iostream>
#include "calculadora.h"
int main() { std::cout << somar(2, 3) << '\n'; }
```

### 2. Usar uma biblioteca

```sh
cppm packages          # lista as bibliotecas com configuração pronta
cppm add fmt           # instala e já configura includes e linkagem
cppm add json spdlog   # várias de uma vez
```

```cpp
#include <fmt/core.h>
int main() { fmt::print("Olá, {}!\n", "mundo"); }
```

Não é preciso editar o `CMakeLists.txt`: o cppm liga as dependências ao alvo
`cppm_dependencies`. `cppm ui` abre um catálogo pesquisável.

### 3. Receber argumentos

```cpp
#include <iostream>
int main(int argc, char** argv) {
    for (int i = 1; i < argc; ++i) std::cout << "arg: " << argv[i] << '\n';
}
```

```sh
cppm run dev -- um dois
```

### 4. Escrever testes

O teste inicial (`app_smoke`) só verifica se o programa termina com código 0.
Para testes de verdade, instale um framework só para os testes:

```sh
cppm add -D catch2
```

Separe a lógica em funções (como `somar` acima) e teste-as:

```cpp
// tests/unit.cpp
#include <catch2/catch_test_macros.hpp>
#include "calculadora.h"

TEST_CASE("somar") {
    REQUIRE(somar(2, 3) == 5);
}
```

No `CMakeLists.txt`, dentro do bloco `if(BUILD_TESTING)`:

```cmake
    add_executable(unit_tests tests/unit.cpp src/calculadora.cpp)
    target_include_directories(unit_tests PRIVATE include)
    target_link_libraries(unit_tests PRIVATE
        cppm_dependencies cppm_dev_dependencies Catch2::Catch2WithMain)
    add_test(NAME unit COMMAND unit_tests)
```

`cppm run test` roda os dois testes; `cppm watch test` repete a cada alteração.

### 5. Ir além

- Interface gráfica ou jogo: `cppm add sdl3`, `cppm add raylib`, `cppm add imgui`.
- HTTP: comece um projeto com `cppm init --template drogon`.
- Navegador: comece um projeto com `cppm init --template wasm`.

