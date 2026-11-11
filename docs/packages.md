# Bibliotecas e bancos de dados

O cppm instala bibliotecas do Conan 2 e conecta seus includes e targets ao
CMake do projeto. Use `cppm packages` para consultar as versões padrão do
binário instalado. Elas são versões iniciais fixas, não uma promessa de usar
sempre o lançamento mais recente.

## Índice

- [Instalar, escolher versão e remover](#instalar-escolher-versão-e-remover)
- [Bancos de dados](#bancos-de-dados)
- [Rede e HTTP](#rede-e-http)
- [Interfaces gráficas](#interfaces-gráficas)
- [Matemática, imagens e paralelismo](#matemática-imagens-e-paralelismo)
- [Serialização](#serialização)
- [Testes](#testes)
- [Utilitários](#utilitários)
- [Perfis e opções do Conan](#perfis-e-opções-do-conan)

## Instalar, escolher versão e remover

Execute na pasta do projeto ou use `-C pasta`. Instale uma biblioteca por comando:

```sh
cppm init meu-app
cd meu-app
cppm install fmt
cppm add fmt@11.0.2
cppm remove fmt
```

`install nome` e `add nome` adicionam uma dependência direta. Também é possível
instalar várias em uma chamada, como `cppm install fmt json eigen`. Sem nome,
`cppm install` instala o manifesto existente. Os nomes não diferenciam
maiúsculas: `Boost`, `OpenCV` e `Qt` funcionam. A versão após `@` é a versão da
biblioteca cliente, não a versão de um servidor de banco.

O manifesto recebe o nome canônico e a versão exata. O lockfile registra o grafo
resolvido. Bibliotecas fora do catálogo continuam disponíveis pelo nome da
receita Conan, com resolução automática de versão.

## Bancos de dados

| Banco | Comando | Receita padrão | Header |
| --- | --- | --- | --- |
| SQLite | `cppm install sqlite` | `sqlite3/3.54.0` | `<sqlite3.h>` |
| MySQL | `cppm install mysql` | `mariadb-connector-c/3.4.8` | `<mysql.h>` |
| MariaDB | `cppm install mariadb` | `mariadb-connector-c/3.4.8` | `<mysql.h>` |
| PostgreSQL | `cppm install postgres` | `libpq/17.11` | `<libpq-fe.h>` |
| Redis | `cppm install redis` | `hiredis/1.3.0` | `<hiredis/hiredis.h>` |
| MongoDB | `cppm install mongodb` | `mongo-c-driver/2.5.8` | `<mongoc/mongoc.h>` |

Os comandos instalam os drivers que o código C++ usa. SQLite pode abrir um
arquivo local ou uma base em memória. MySQL, MariaDB, PostgreSQL, Redis e MongoDB
se conectam a servidores configurados separadamente.

MySQL e MariaDB usam o mesmo MariaDB Connector/C, compatível com os dois
servidores. Instalar ambos os aliases mantém uma única dependência direta.
`postgresql` e `postgress` também selecionam `libpq`; `mongo` seleciona o
mesmo cliente de `mongodb`. Remoção aceita esses mesmos aliases.

### SQLite: uma consulta completa

Instale `sqlite`, coloque este código em `src/main.cpp` e execute `cppm run dev`:

```cpp
#include <sqlite3.h>
#include <iostream>

int main() {
    sqlite3* database = nullptr;
    if (sqlite3_open(":memory:", &database) != SQLITE_OK) {
        if (database) sqlite3_close(database);
        return 1;
    }
    sqlite3_stmt* query = nullptr;
    if (sqlite3_prepare_v2(database, "SELECT 6 * 7", -1, &query, nullptr) != SQLITE_OK) {
        sqlite3_close(database);
        return 2;
    }
    const bool found = sqlite3_step(query) == SQLITE_ROW;
    if (found) std::cout << sqlite3_column_int(query, 0) << '\n';
    sqlite3_finalize(query);
    sqlite3_close(database);
    return found ? 0 : 3;
}
```

### PostgreSQL: conectar usando o ambiente

O libpq aceita `PGHOST`, `PGPORT`, `PGDATABASE` e `PGUSER`. Configure suas
credenciais pelo ambiente ou pelo mecanismo de autenticação do servidor.

```cpp
#include <libpq-fe.h>
#include <iostream>

int main() {
    PGconn* connection = PQconnectdb("");
    if (!connection || PQstatus(connection) != CONNECTION_OK) {
        if (connection) {
            std::cerr << PQerrorMessage(connection);
            PQfinish(connection);
        }
        return 1;
    }
    PGresult* result = PQexec(connection, "SELECT 6 * 7");
    const bool ok = result && PQresultStatus(result) == PGRES_TUPLES_OK;
    if (ok && PQntuples(result) > 0) std::cout << PQgetvalue(result, 0, 0) << '\n';
    if (result) PQclear(result);
    PQfinish(connection);
    return ok ? 0 : 2;
}
```

Os exemplos em `examples/databases/` verificam inicialização dos clientes,
linkagem, parsing Redis e BSON sem exigir servidores em execução. Não são testes
de conexão ou de autenticação com um servidor real.

## Rede e HTTP

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| cpp-httplib | `cppm install cpp-httplib` | `0.62.0` | `<httplib.h>` |
| Asio standalone | `cppm install asio` | `1.38.2` | `<asio.hpp>` |

`httplib` é alias de `cpp-httplib`. Asio standalone é uma biblioteca independente;
Boost.Asio acompanha o pacote `boost` e usa `<boost/asio.hpp>`.
No Windows, projetos com Asio ou Boost recebem `_WIN32_WINNT=0x0A00` (Windows 10),
o que evita o aviso "Please define _WIN32_WINNT" em cada compilação. Para outro
alvo, configure `-DCPPM_WIN32_WINNT=0x0601`; um valor vazio desativa a definição.

Exemplo de servidor com cpp-httplib:

```cpp
#include <httplib.h>

int main() {
    httplib::Server server;
    server.Get("/health", [](const httplib::Request&, httplib::Response& response) {
        response.set_content("ok", "text/plain");
    });
    return server.listen("127.0.0.1", 8080) ? 0 : 1;
}
```

Execute `cppm run dev` e acesse `http://127.0.0.1:8080/health`. HTTPS depende de
ativar um backend TLS nas opções Conan da biblioteca; instalar o padrão HTTP
não habilita TLS automaticamente.

## Interfaces gráficas

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| Dear ImGui | `cppm install imgui` | `1.92.9b` | `<imgui.h>` |
| Qt 6 | `cppm install qt` | `6.12.0` | `<QCoreApplication>`, `<QWidget>` |
| SDL2 | `cppm install sdl2` | `2.32.10` | `<SDL.h>` |
| SDL3 | `cppm install sdl3` | `3.4.16` | `<SDL3/SDL.h>` |

`dear-imgui` e `dearimgui` são aliases de `imgui`; `qt6` é alias de `qt`.

`sdl2` e `sdl3` selecionam gerações do mesmo pacote Conan `sdl`. Usar apenas
`cppm add sdl` consulta a versão disponível, que pode ser SDL3. Para exemplos
antigos com `#include <SDL.h>`, use `cppm add sdl2`. A
[migração para SDL3](https://wiki.libsdl.org/SDL3/README-migration) também envolve
mudanças de API. Veja exemplos mínimos em [SDL2](../examples/sdl2/main.cpp) e
[SDL3](../examples/sdl3/main.cpp).

Dear ImGui fornece a interface. Para renderizar uma janela, escolha e configure
um backend de plataforma e renderização, como SDL + OpenGL ou GLFW + OpenGL.
Só instalar a biblioteca não cria a janela nem um loop de eventos.

Qt oferece Core, Gui e Widgets conforme as opções da receita. O CMake continua
pertencendo ao projeto: configure os recursos de automação necessários para
classes com `Q_OBJECT`, formulários `.ui` e recursos `.qrc`. Um teste com
`QCoreApplication` verifica Core sem abrir janela; ele não valida distribuição
dos plugins gráficos.

O padrão Qt usa bibliotecas dinâmicas (`qt/*:shared=True`). Para distribuir uma
aplicação, inclua as bibliotecas e plugins Qt necessários no ambiente de destino.
Um perfil Conan pode selecionar a variante estática, que exige uma configuração
de linkagem e plugins própria; ela não é o caminho padrão validado pelo cppm.

O plugin Qt SQL para PostgreSQL fica desativado no padrão
(`qt/*:with_pq=False`), devido à linkagem incompleta do libpq na receita Qt.
Isso não impede usar `cppm install postgres` e a API libpq na mesma aplicação.
Outros plugins SQL dependem das opções da receita.

No Linux, o cppm compila o pacote Qt com o dialeto GNU do padrão escolhido,
necessário para as extensões de inteiros usadas pelo framework. O padrão C++
da aplicação continua sendo o declarado no manifesto.

Bibliotecas gráficas podem exigir SDKs e pacotes de desenvolvimento da
plataforma. A receita Conan informa os requisitos ausentes. O cppm não executa
instalações privilegiadas do sistema.

## Matemática, imagens e paralelismo

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| OpenCV | `cppm install opencv` | `4.14.0` | `<opencv2/core.hpp>` |
| Eigen | `cppm install eigen` | `3.4.0` | `<Eigen/Dense>` |
| GLM | `cppm install glm` | `1.0.3` | `<glm/glm.hpp>` |
| oneTBB | `cppm install onetbb` | `2023.1.0` | `<oneapi/tbb/parallel_for.h>` |

`opencv4`, `tbb` e `one-tbb` são aliases. Eigen 3.4.0 é o padrão para manter
compatibilidade com o Eigen exigido por OpenCV 4.14.0. Use `eigen@5.0.1` em um
projeto que precise dessa API e cujo grafo seja compatível.

O padrão OpenCV do cppm desativa a integração FFmpeg para começar com imagens
sem compilar a cadeia completa de codecs de áudio/vídeo. Para vídeo por FFmpeg,
adicione `opencv/*:with_ffmpeg=True` à seção `[options]` do perfil Conan e use
esse perfil tanto na instalação quanto no build.

No Linux, o cppm habilita `opencv/*:with_v4l=True` por padrão para webcams V4L2.
Esse backend usa os headers de vídeo do Linux e não precisa de FFmpeg. Um perfil
pode desabilitá-lo explicitamente. Pacotes anteriores com `with_v4l=False`
precisam de uma nova compilação do OpenCV; as outras dependências são reutilizadas
do armazenamento compartilhado. O padrão não altera backends de Windows/macOS.

Para conferir a abertura da câmera, teste o retorno antes de usar os frames:

```cpp
#include <opencv2/videoio.hpp>
#include <iostream>

int main() {
    cv::VideoCapture camera(0);
    if (!camera.isOpened()) {
        std::cerr << "Nao foi possivel abrir a camera\n";
        return 1;
    }
}
```

Exemplo de paralelismo com oneTBB:

```cpp
#include <oneapi/tbb/parallel_for.h>
#include <atomic>
#include <iostream>

int main() {
    std::atomic<int> count{0};
    oneapi::tbb::parallel_for(0, 42, [&](int) { ++count; });
    std::cout << count << '\n';
}
```

## Serialização

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| nlohmann/json | `cppm install json` | `3.11.3` | `<nlohmann/json.hpp>` |
| Protobuf | `cppm install protobuf` | `7.36.2` | `<google/protobuf/message.h>` |

`json` seleciona `nlohmann_json`; `protocol-buffers` seleciona `protobuf`.
O runtime Protobuf serializa e lê mensagens geradas. Para tipos próprios,
adicione o schema `.proto` e sua etapa de geração ao CMake. A instalação do
pacote não transforma arquivos `.proto` em código automaticamente.

### Gerar C++ a partir de um schema

Crie `proto/pessoa.proto`:

```proto
syntax = "proto3";
message Pessoa { string nome = 1; }
```

Depois de instalar Protobuf, acrescente ao final do `CMakeLists.txt` do projeto
`meu-app`:

```cmake
add_custom_command(
    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.cc"
           "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.h"
    COMMAND protobuf::protoc
    ARGS "--cpp_out=${CMAKE_CURRENT_BINARY_DIR}"
         "--proto_path=${CMAKE_CURRENT_SOURCE_DIR}/proto"
         "${CMAKE_CURRENT_SOURCE_DIR}/proto/pessoa.proto"
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/proto/pessoa.proto"
    VERBATIM)
target_sources(meu-app PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.cc")
target_include_directories(meu-app PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
```

Use o tipo gerado em `src/main.cpp` e execute `cppm run dev`:

```cpp
#include <pessoa.pb.h>
#include <string>

int main() {
    Pessoa pessoa;
    pessoa.set_nome("cppm");
    std::string bytes;
    if (!pessoa.SerializeToString(&bytes)) return 1;
    Pessoa decoded;
    return decoded.ParseFromString(bytes) && decoded.nome() == "cppm" ? 0 : 2;
}
```

O target `protobuf::protoc` usa o executável do pacote selecionado pelo cppm,
evitando misturar o runtime com um compilador diferente instalado no PATH.

## Testes

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| Catch2 | `cppm install catch2` | `3.7.1` | `<catch2/catch_test_macros.hpp>` |
| Google Test | `cppm install googletest` | `1.18.0` | `<gtest/gtest.h>` |

`googletest`, `google-test` e `google_test` selecionam `gtest`. Crie os testes e
registre os executáveis no CTest do seu `CMakeLists.txt`; `cppm run test` compila
e executa o CTest.

Para um executável de testes, você pode escolher explicitamente os targets
`Catch2::Catch2WithMain` ou `GTest::gtest_main`. O projeto inicial já contém um
teste de execução simples, independente desses frameworks.

## Utilitários

| Biblioteca | Comando | Versão padrão | Header |
| --- | --- | --- | --- |
| Boost | `cppm install boost` | `1.91.0` | `<boost/filesystem.hpp>` |
| fmt | `cppm install fmt` | `11.0.2` | `<fmt/format.h>` |
| spdlog | `cppm install spdlog` | `1.15.0` | `<spdlog/spdlog.h>` |
| zlib | `cppm install zlib` | `1.3.1` | `<zlib.h>` |

As bibliotecas usam as próprias licenças, opções e requisitos de plataforma.
O catálogo é um ponto de partida: as dependências transitivas continuam sob
resolução e validação do Conan.

## Perfis e opções do Conan

Use um perfil explícito para opções de bibliotecas e compilação. Exemplo de
perfil `profiles/dev`, criado no seu projeto:

```ini
include(default)

[conf]
tools.build:jobs=4

[options]
cpp-httplib/*:with_openssl=True
```

```sh
cppm install cpp-httplib --profile ./profiles/dev
cppm run dev --profile ./profiles/dev
```

Use o mesmo perfil ao instalar e compilar. Nesta versão, o perfil é aplicado
aos contextos host e build; cross-compilation não está implementado.

Para uma receita com nomes CMake específicos, passe o override explícito:

```sh
cppm add pacote@1.2.3 --cmake-package Pacote --target Pacote::Pacote
```

Se duas bibliotecas exigirem versões incompatíveis de uma dependência comum,
o Conan recusa o grafo. Escolha versões e opções compatíveis em vez de tratar
o lockfile como um mecanismo que elimina conflitos.
