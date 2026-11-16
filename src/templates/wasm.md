# {name}

C++ rodando no navegador com WebAssembly (Emscripten), criado com
`cppm init --template wasm`.

```sh
cppm run dev                    # compila e serve em http://127.0.0.1:8080
cppm run dev -- --port 9090     # outra porta
cppm watch                      # recompila ao salvar; atualize a página depois
cppm run test                   # executa o programa com Node.js
cppm run build                  # Release em .cppm/build/wasm-release/bin/
```

## Preparar o ambiente

O instalador completo do cppm para Windows já instala o Emscripten e um Node.js
privado. Em outros sistemas, instale o
[Emscripten SDK](https://emscripten.org/docs/getting_started/downloads.html) e rode
`emsdk activate latest` uma vez. O cppm encontra o SDK sozinho (`CPPM_EMSDK`, `EMSDK`,
`PATH`, `~/emsdk` ou `~/.local/share/cppm/emsdk`); não é preciso carregar scripts de
ambiente. `cppm doctor` mostra o SDK escolhido e compila um módulo de teste.
O servidor de desenvolvimento usa Python 3.

## Estrutura

```text
{name}/
  cppm.toml         # target = "wasm"
  CMakeLists.txt    # opções do Emscripten (target_link_options)
  src/main.cpp      # código C++
  web/shell.html    # página HTML onde o módulo é carregado
```

O build gera três arquivos que andam juntos: `{name}.html`, `{name}.js` e `{name}.wasm`.
Tudo que o `main` imprime com `std::cout` aparece no `<pre id="output">` da página.

## Próximos passos

### 1. Mudar a página

Edite `web/shell.html` livremente (HTML, CSS, botões). Só mantenha o objeto
`var Module = {...}` e a linha `{{{ SCRIPT }}}`, que é onde o Emscripten insere o JS.

### 2. Chamar funções C++ a partir do JavaScript

No `CMakeLists.txt`, acrescente no final:

```cmake
target_link_options(${PROJECT_NAME} PRIVATE "-lembind")
```

No C++:

```cpp
#include <emscripten/bind.h>
#include <string>

int somar(int a, int b) { return a + b; }
std::string saudacao(const std::string& nome) { return "Olá, " + nome + "!"; }

EMSCRIPTEN_BINDINGS(app) {
    emscripten::function("somar", &somar);
    emscripten::function("saudacao", &saudacao);
}
```

No `web/shell.html`, use as funções depois que o módulo carregar:

```html
<button id="botao">Somar</button>
<script>
    var Module = {
        print: function (text) { document.getElementById('output').textContent += text + '\n'; },
        printErr: function (text) { document.getElementById('output').textContent += text + '\n'; },
        onRuntimeInitialized: function () {
            document.getElementById('botao').onclick = function () {
                alert(Module.somar(2, 3) + ' / ' + Module.saudacao('Ana'));
            };
        }
    };
</script>
```

Classes também podem ser expostas com `emscripten::class_`; veja a
[documentação do Embind](https://emscripten.org/docs/porting/connecting_cpp_and_javascript/embind.html).

### 3. Mexer na página a partir do C++

Com `-lembind` (passo 2), `emscripten::val` acessa qualquer objeto JavaScript:

```cpp
#include <emscripten/val.h>
#include <string>

int main() {
    auto document = emscripten::val::global("document");
    auto titulo = document.call<emscripten::val>("querySelector", std::string("h1"));
    titulo.set("textContent", emscripten::val::u8string("Título trocado pelo C++"));
}
```

Use `emscripten::val::u8string(...)` (ou um `std::string`) para textos com acento;
um literal `"..."` direto é convertido como Latin-1 e os acentos saem trocados.

Para trechos curtos de JS dentro do C++, sem embind, use `EM_JS`:

```cpp
#include <emscripten/emscripten.h>

EM_JS(void, log_js, (const char* texto), {
    console.log(UTF8ToString(texto));
});
```

> `cppm run test` executa o programa no Node.js, onde não existe `document`.
> Proteja o código de página com `if (!document.isUndefined())` ou ajuste o teste.

### 4. Animação e jogos (loop principal)

No navegador, o `main` não pode ficar preso em um `while (true)`. Registre uma
função chamada a cada quadro:

```cpp
#include <emscripten/emscripten.h>

void quadro() {
    // atualizar estado e desenhar
}

int main() {
    emscripten_set_main_loop(quadro, 0, true);   // 0 = taxa do monitor
}
```

Para gráficos, o Emscripten traz SDL2 pronto. No `CMakeLists.txt`:

```cmake
target_compile_options(${PROJECT_NAME} PRIVATE "-sUSE_SDL=2")
target_link_options(${PROJECT_NAME} PRIVATE "-sUSE_SDL=2")
```

E no `web/shell.html`, adicione `<canvas id="canvas"></canvas>` e a propriedade
`canvas: document.getElementById('canvas')` no objeto `Module`. Use `#include <SDL.h>`
normalmente e desenhe dentro de `quadro()`. Programas gráficos não rodam no Node.js;
nesse caso remova ou adapte o teste `wasm_smoke` do `CMakeLists.txt`.

### 5. Arquivos (imagens, textos, níveis)

Coloque-os em `assets/` e empacote no build:

```cmake
target_link_options(${PROJECT_NAME} PRIVATE
    "--preload-file" "${CMAKE_CURRENT_SOURCE_DIR}/assets@/assets")
```

No C++, leia normalmente: `std::ifstream file("/assets/mensagem.txt");`.
O build passa a gerar também `{name}.data`, que deve ser publicado junto.

### 6. Bibliotecas

Dependências Conan (`cppm add`) ainda não funcionam no target WASM. Alternativas:

- Bibliotecas só de headers (por exemplo nlohmann/json, glm): copie os headers para
  `include/`.
- Ports do Emscripten, ativados por flag: `-sUSE_SDL=2`, `-sUSE_SDL_IMAGE=2`,
  `-sUSE_ZLIB=1` e outros — veja `emcc --show-ports`.
- Código-fonte de bibliotecas pequenas pode ir direto em `src/`.

### 7. Publicar

```sh
cppm run build
```

Copie o conteúdo de `.cppm/build/wasm-release/bin/` (`.html`, `.js`, `.wasm` e `.data`,
se houver) para qualquer hospedagem de site estático: GitHub Pages, Netlify, Vercel,
um servidor nginx. Renomeie `{name}.html` para `index.html` se quiser que ele abra na
raiz. A página precisa ser servida por HTTP; abrir o arquivo direto (`file://`) não funciona.

