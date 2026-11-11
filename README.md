# cppm

O básico de um projeto C++ em poucos comandos. CLI em Rust, compilação com
CMake/Ninja e dependências com Conan 2.

```sh
cppm init meu-app
cd meu-app
cppm add fmt
cppm run dev
cppm run build
cppm run test
```

Para começar com um servidor HTTP ou WebAssembly:

```sh
cppm init minha-api --template drogon
cppm init meu-wasm --template wasm
```

O template Drogon já inclui a dependência, as rotas `/` e `/health` com resposta
JSON e um teste HTTP. `cppm run dev` inicia a API em `http://127.0.0.1:8080`.
Use `cppm run dev -- --port 9090` para escolher outra porta.

O template WASM usa Emscripten. O cppm localiza o SDK instalado automaticamente,
inclusive o SDK deste checkout em `.qa/emsdk`, sem exigir `source` a cada terminal.
Use `CPPM_EMSDK` para selecionar outro diretório. `cppm run dev` gera HTML, JS
e WASM e inicia um servidor local. `cppm run test`
executa o módulo com Node.js. Python 3 é usado pelo servidor de desenvolvimento
WASM e pelo teste HTTP Drogon. Dentro de um projeto WASM, `cppm doctor` verifica
Emscripten, Node.js e Python 3 e compila e executa um módulo WebAssembly de teste.

Para desenvolver sem repetir comandos:

```sh
cppm watch                       # recompila e reinicia o script dev ao salvar
cppm watch test                  # executa os testes após alterações
cppm watch dev -- --port 9090    # porta da API Drogon ou do servidor WASM
cppm clean --dry-run             # mostra os builds e bytes que seriam removidos
cppm clean debug                 # remove apenas o build Debug
```

O `watch` continua esperando após erros de compilação e encerra os processos
anteriores antes de reiniciar. Em WASM, atualize a página depois do rebuild.
Builds repetidos reutilizam dependências e a configuração CMake quando seus
arquivos, perfil e ambiente permanecem válidos. `cppm build --fresh` força a
preparação completa; `--verbose` mostra os tempos de cada etapa.

O projeto tem validação local em Linux e um executável Windows x64 com testes
de CLI no Wine. O CI Windows dedicado ao instalador verifica downloads,
instalação, PATH, build C++ com MSVC e desinstalação. `cppm` é o nome provisório.

## Documentação

- [Guia organizado: projeto, scripts, builds, templates e problemas comuns](docs/guide.md)
- [Catálogo de bibliotecas, bancos de dados e exemplos](docs/packages.md)
- [Instalação Linux e Windows](docs/installation.md)
- [Guia Windows: ferramentas, DLLs, diagnóstico e validação](docs/windows.md)
- [Arquitetura e decisões de implementação](docs/architecture.md)
- [Medições de desempenho dos builds incrementais](docs/performance.md)
- [Roadmap: o que falta para a paridade com o pnpm](docs/roadmap.md)

## Instalação

Instale a CLI diretamente pelo GitHub, sem Rust:

```sh
# Linux x64 ou ARM64
curl -fsSL https://raw.githubusercontent.com/GaussNoob/cppm/main/scripts/bootstrap.sh -o install-cppm.sh
sh install-cppm.sh
```

```powershell
# Windows x64 (PowerShell)
Invoke-WebRequest -UseBasicParsing https://raw.githubusercontent.com/GaussNoob/cppm/main/scripts/bootstrap.ps1 -OutFile install-cppm.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\install-cppm.ps1
```

Os scripts baixam a última release e verificam o SHA-256 antes de instalar.
No Windows, acrescente `-WithDependencies` para preparar também as ferramentas
C++ e WebAssembly. Abra um terminal novo após a instalação. No Linux, inclua
`~/.local/bin` no PATH se necessário; no fish: `fish_add_path ~/.local/bin`.

O cppm consulta novas versões em segundo plano e avisa ao usar a CLI ou a
interface interativa. Para consultar imediatamente ou aplicar a atualização:

```sh
cppm update --check
cppm update
```

O aviso usa cache de seis horas; a atualização é aplicada pelo comando acima.
`--offline`, `CPPM_NO_UPDATE_CHECK=1` e ambientes de CI desativam a consulta
automática. Veja [instalação, atualização e publicação](docs/installation.md).
Os scripts remotos e a consulta de versões passam a funcionar após a primeira
release publicada pelo novo workflow `release.yml`, que inclui o arquivo `VERSION`.

Baixe os pacotes na [última release](https://github.com/GaussNoob/cppm/releases/latest).
No Windows, execute `cppm-0.5.0-windows-x86_64-setup.exe`: o assistente instala a
CLI, Python, Conan, CMake, Ninja, Emscripten e seu Node.js privado, prepara MSVC e Windows SDK se faltarem e
configura o PATH. Depois, abra um terminal novo e use `cppm`. A etapa do
compilador pode pedir administrador e baixar vários GB.

Também há pacotes portáteis Windows e instalador por usuário no Linux. As
instruções estão em [docs/installation.md](docs/installation.md).

Requisitos: Rust 1.89 ou superior para compilar a CLI, CMake 3.24 ou superior,
Ninja e um compilador C++. Conan 2 é necessário quando o projeto tem
dependências; a integração foi validada com Conan 2.34.0.

```sh
uv tool install 'conan>=2.34,<3'
cargo install --path . --locked
cppm doctor
```

Também é possível usar `pipx install conan`. Conan precisa estar no `PATH`.
Depois de compilada, a CLI não depende do Rust instalado na máquina.

Neste workspace já existe um ambiente virtual com Conan para desenvolvimento:

```sh
cd ~/Projetos/cppm
cargo build --release --locked
cppm --help
```

Os comandos deste workspace estão disponíveis permanentemente por links em
`~/.local/bin`, que já está no `PATH` do fish:

```text
~/.local/bin/cppm → ~/Projetos/cppm/target/release/cppm
~/.local/bin/conan → ~/Projetos/cppm/.venv/bin/conan
```

Os links continuam válidos enquanto o projeto permanecer nesse diretório.
Um `export PATH=...` feito no terminal só altera aquela sessão.

Há um projeto de exemplo em `examples/hello`, com `fmt` e lockfile versionado:

```sh
cppm -C examples/hello run dev
```

## Interface interativa

```sh
cppm ui
cppm -C examples/hello ui
cppm --offline ui
```

`cppm ui` (também `cppm tui` ou `cppm interactive`) abre uma interface de
terminal com tema escuro, quatro abas e navegação pelo teclado:

- **Visão geral:** projeto, padrão C++, perfil, lockfile e atalhos.
- **Explorar:** bibliotecas por categoria, descrições, aliases, seleção em lote
  e pesquisa por nome ou assunto. Ao abrir a aba, consulta os remotos Conan
  habilitados em segundo plano, incluindo receitas fora do catálogo local.
  Sem Conan ou sem conexão, as sugestões locais continuam disponíveis.
- **Dependências:** versões declaradas no projeto, correspondência com o
  lockfile, sincronização e remoção.
- **Comandos:** scripts do manifesto, builds Debug/Release, execução, testes,
  integração com o editor, diagnóstico e armazenamento compartilhado.

Use `Tab` ou `1`–`4` para trocar de aba, `/` para buscar, setas para navegar
e `Enter` para abrir uma ação. Na aba Explorar, `Espaço` seleciona bibliotecas
e `Enter` instala o conjunto; `a` permite informar nomes ou `pacote@versão`,
inclusive receitas ainda não listadas. `c` alterna categorias, `f` alterna
filtros (todos, com padrão cppm, sem padrão cppm ou fora do projeto), `r`
atualiza a consulta e `x` limpa a seleção. `Ctrl+P` abre a paleta de comandos,
`?` mostra a ajuda e `q` sai.

Cada ação apresenta uma prévia do comando para confirmar ou cancelar. Durante
a execução, a saída aparece no terminal; pressione `Enter` ao final para voltar
à interface. Perfil, armazenamento, offline e lockfile congelado seguem as
opções globais da CLI. Com `--offline`, a consulta usa apenas receitas no cache
local. O catálogo lista receitas `nome/versão`; a disponibilidade de binários e
a compatibilidade com o perfil são determinadas na instalação pelo Conan.

Também funciona fora de um projeto para explorar bibliotecas e executar
diagnósticos. A aba Comandos oferece a criação de um projeto na pasta atual.
É necessário um terminal interativo com pelo menos 52 colunas e 19 linhas.

## Comandos

| Comando                     | Comportamento                                            |
| --------------------------- | -------------------------------------------------------- |
| `cppm ui`                  | Abre o explorador interativo de bibliotecas, projeto e comandos |
| `cppm update --check`      | Consulta a última versão estável sem instalar |
| `cppm update`              | Atualiza o próprio cppm com verificação SHA-256 |
| `cppm init [pasta]`         | Cria manifesto, fonte inicial, CMake e teste de execução |
| `cppm add fmt`              | Adiciona a versão padrão do catálogo e instala           |
| `cppm add fmt@11.0.2`       | Adiciona uma versão exata                                |
| `cppm install fmt`          | Atalho para adicionar uma dependência                    |
| `cppm install fmt json eigen` | Instala vários pacotes em uma operação; preserva manifesto e lockfile se falhar |
| `cppm install`              | Instala as dependências do manifesto                     |
| `cppm add -D catch2`        | Adiciona em `[dev-dependencies]`: só os testes linkam     |
| `cppm remove fmt`           | Remove a dependência direta e atualiza o lockfile        |
| `cppm ci`                   | Instala exatamente o `cppm.lock`; falha em vez de regravá-lo |
| `cppm list`                 | Lista as dependências diretas (`ls`); `--all` mostra a árvore resolvida |
| `cppm why zlib`             | Mostra quais dependências trazem um pacote para o projeto |
| `cppm licenses`             | Agrupa as bibliotecas do grafo pela licença declarada     |
| `cppm outdated`             | Compara as dependências diretas com as versões disponíveis |
| `cppm upgrade [pacote]`     | Atualiza dentro da versão major (`up`); `--latest` cruza majors |
| `cppm run`                  | Lista os scripts                                         |
| `cppm run dev`              | Compila em Debug e executa                               |
| `cppm run build`            | Compila em Release                                       |
| `cppm run test`             | Compila e executa CTest                                  |
| `cppm watch [script]`       | Recompila e reinicia ao salvar; usa `dev` por padrão      |
| `cppm watch test`           | Reexecuta os testes ao alterar fontes ou configuração    |
| `cppm build --release -j 4` | Build direto, com quatro tarefas                         |
| `cppm build --fresh`        | Revalida dependências e recria o cache CMake              |
| `cppm clean --dry-run`      | Mostra arquivos e bytes dos builds sem removê-los         |
| `cppm clean [modo]`         | Remove builds: `all` (padrão), `debug`, `release` ou `obf` |
| `cppm build obf`            | Release separado com ofuscador LLVM incluído |
| `cppm test obf`             | Compila e testa o executável protegido                   |
| `cppm configure`            | Atualiza CMake e os caminhos do editor sem compilar a aplicação |
| `cppm vscode --workspace ..` | Cria tarefas de build/execução na pasta pai aberta no VS Code |
| `cppm exec -- argumento`    | Compila e repassa argumentos à aplicação                 |
| `cppm test -- -R app_smoke` | Repassa argumentos ao CTest                              |
| `cppm packages`             | Lista versões e targets com configuração pronta          |
| `cppm doctor`               | Verifica versões, compila e executa um programa C++       |
| `cppm doctor --json`        | Retorna todos os diagnósticos em um relatório JSON        |
| `cppm store path`           | Mostra o armazenamento compartilhado das bibliotecas     |
| `cppm store status`         | Mostra pacotes, projetos e bytes economizados por compartilhamento |
| `cppm store projects`       | Lista os projetos registrados no armazenamento          |
| `cppm store dedupe`         | Compartilha arquivos idênticos entre pacotes e versões   |
| `cppm store prune`          | Remove pacotes gerenciados sem uso e sobras temporárias  |

`-C pasta` escolhe o diretório de trabalho. Dentro de um projeto, a busca pelo
manifesto também funciona a partir de subdiretórios.

## Build protegido

```sh
cppm build obf
cppm exec obf -- argumento
cppm test obf
```

O preset `obf` inclui o Obfuscator-LLVM: prepara o plugin automaticamente no
primeiro uso e ativa mascaramento de strings locais elegíveis, achatamento de
fluxo de controle, substituição de instruções e divisão de blocos. O Release
fica em `.cppm/build/obf/bin/`, com símbolos reduzidos e caminhos do projeto
remapeados. Os fontes e o CMake do usuário são preservados.

Requer Clang e o SDK de desenvolvimento LLVM correspondente (18–23), incluindo
`llvm-config`. O cppm seleciona o Clang do SDK e, quando usa o perfil Conan
`default`, deriva um perfil Clang local sem sobrescrever o original. O plugin
é compilado dos fontes incluídos no executável e reutilizado em `.cppm/tools/`;
não exige download à parte e pode ser preparado offline. Validado em Linux com
Clang/LLVM 23.1.4. O host Windows/MSVC não suporta o plugin incluído.

Para reduzir apenas os metadados usando GCC, Clang ou MSVC:

```sh
cppm build obf --obf-basic
```

Para escolher outro SDK ou continuar usando um plugin externo:

```sh
CPPM_LLVM_CONFIG=/caminho/llvm-config cppm build obf
CXX=clang++ cppm build obf --obf-plugin /caminho/plugin.so
```

A interface `cppm ui` oferece os dois modos. Ofuscação não impede toda
engenharia reversa; o modo básico não aplica passes de transformação. Veja
requisitos, escopo e validação em [Build protegido e plugins LLVM](docs/guide.md#build-protegido-e-plugins-llvm).
O componente incluído é fixado em uma revisão do
[eshard/obfuscator-llvm](https://github.com/eshard/obfuscator-llvm); sua licença e
as adaptações estão em `vendor/llvm-obfuscator/` e `THIRD_PARTY_NOTICES.txt`.

## Manifesto

```toml
[package]
name = "meu-app"
version = "0.1.0"
cpp-standard = 20

[dependencies]
fmt = "11.0.2"

[scripts]
dev = ["cppm", "exec", "--"]
build = ["cppm", "build", "--release"]
test = ["cppm", "test", "--"]
```

`add`, `remove` e `upgrade` editam somente as entradas alteradas: comentários,
ordem das chaves e formatação do restante do `cppm.toml` são preservados.

### Dependências de desenvolvimento

Frameworks de teste e ferramentas não precisam ser linkados na aplicação:

```sh
cppm add -D catch2
```

```toml
[dependencies]
fmt = "11.0.2"

[dev-dependencies]
catch2 = "3.7.1"
```

O cppm gera dois targets CMake: `cppm_dependencies`, usado pela aplicação, e
`cppm_dev_dependencies`, para os executáveis de teste:

```cmake
add_executable(unit tests/unit.cpp)
target_link_libraries(unit PRIVATE cppm_dependencies cppm_dev_dependencies)
add_test(NAME unit COMMAND unit)
```

Os headers de uma dependência de desenvolvimento não ficam visíveis para a
aplicação. Repetir `cppm add` com ou sem `-D` move o pacote de seção. As duas
seções entram no mesmo grafo Conan e no mesmo `cppm.lock`.

Scripts em arrays são executados diretamente, sem shell, em Linux e Windows.
Scripts em string continuam disponíveis: usam `sh` no Linux e Windows PowerShell
no Windows, com a sintaxe de cada shell. Os scripts sempre executam na raiz do
projeto. Argumentos após `--` são acrescentados ao comando:

```sh
cppm run dev -- "um argumento com espaços"
```

O `--` dentro do script `dev` separa as opções do cppm dos argumentos da
aplicação. Opções globais de perfil, armazenamento, offline e lockfile são herdadas por comandos
cppm chamados pelos scripts. O status de saída do programa ou script chega ao
terminal e ao CI.

## Dependências

Bibliotecas publicadas nos remotos Conan não precisam entrar em um catálogo
local. Sem versão explícita, o cppm pede ao Conan uma versão estável e grava
o resultado exato no manifesto. Os metadados da receita instalada fornecem
o nome do pacote CMake e o target de linkagem.

```sh
cppm install sqlite
cppm install mysql
cppm install postgres
cppm install redis
cppm install mongodb
cppm install json
cppm install curl
cppm install sdl
cppm install sfml
cppm install raylib
```

| Comando | Header de exemplo |
| --- | --- |
| `cppm install sqlite` | `<sqlite3.h>` |
| `cppm install json` | `<nlohmann/json.hpp>` |
| `cppm install curl` | `<curl/curl.h>` |
| `cppm install sdl2` | `<SDL.h>` |
| `cppm install sdl3` | `<SDL3/SDL.h>` |
| `cppm install SFML` | `<SFML/Graphics.hpp>` |
| `cppm install raylib` | `<raylib.h>` |

`sqlite` é alias de `sqlite3`, `curl` de `libcurl`, `json` de `nlohmann_json`
e `googletest` de `gtest`. Os nomes na CLI são normalizados para minúsculas,
então `SFML` também funciona. `sdl2` e `sdl3` selecionam versões padrão das duas
gerações do pacote `sdl`. O nome `sdl` consulta a versão disponível no Conan e
pode selecionar SDL3. Para código que usa `SDL.h`, escolha `cppm add sdl2`.
SDL2 e SDL3 têm APIs diferentes; trocar somente o include não migra todo programa.
Veja os exemplos mínimos em [examples/sdl2](examples/sdl2/main.cpp) e
[examples/sdl3](examples/sdl3/main.cpp).

O catálogo inclui bancos de dados, Boost, cpp-httplib, Asio, Dear ImGui, Qt,
Catch2, Google Test, OpenCV, Eigen, Protobuf, GLM e oneTBB, além dos utilitários
iniciais. Todos têm versões padrão fixas, listadas por `cppm packages`.
Use `pacote@versão` para
escolher explicitamente outra versão publicada. A biblioteca padrão do C++,
como `iostream`, `vector` e `filesystem`, já acompanha o compilador.

```sh
cppm install boost
cppm install cpp-httplib
cppm install asio
cppm install imgui
cppm install qt
cppm install catch2
cppm install googletest
cppm install opencv
cppm install eigen
cppm install protobuf
cppm install glm
cppm install onetbb
```

São exemplos para escolher conforme o projeto; instale uma biblioteca por
comando. Os drivers de MySQL/MariaDB, PostgreSQL, Redis e MongoDB se conectam a
servidores configurados separadamente. SQLite também pode operar em arquivo
local ou em memória. Veja os headers, aliases, requisitos e exemplos em
[Bibliotecas e bancos de dados](docs/packages.md).

### JSON: nlohmann/json

O JSON padrão do catálogo é a biblioteca externa **nlohmann/json**. Os dois
comandos abaixo instalam o mesmo pacote `nlohmann_json`:

```sh
cppm install json
# ou: cppm install nlohmann_json
```

```cpp
#include <iostream>
#include <nlohmann/json.hpp>

int main() {
    nlohmann::json dados = {{"nome", "cppm"}, {"versao", 2}};
    std::cout << dados.dump(2) << '\n';
}
```

O manifesto guarda `nlohmann_json = "3.11.3"` por padrão, e a integração usa
os metadados do Conan para configurar o target CMake `nlohmann_json::nlohmann_json`.

### Requisitos e ajustes por biblioteca

SDKs e bibliotecas de sistema exigidos por receitas gráficas, como SDL, SFML
e raylib, continuam sendo requisitos da plataforma. A CLI não executa
instalações privilegiadas de pacotes do sistema.

Se uma receita precisar de um mapeamento CMake específico, você pode sobrescrevê-lo:

```sh
cppm add pacote@1.2.3 --cmake-package Pacote --target Pacote::Pacote
```

Isso gera uma dependência detalhada:

```toml
[dependencies.pacote]
version = "1.2.3"
cmake-package = "Pacote"
targets = ["Pacote::Pacote"]
```

A instalação gera `.cppm/dependencies.cmake`, incluído pelo CMake do projeto.
As bibliotecas são ligadas pelo target de interface `cppm_dependencies`.
O `CMakeLists.txt` pertence ao projeto: você pode adicionar targets, testes,
diretórios e opções. A CLI não o reescreve depois de `init`. `exec` espera que
o target executável tenha o mesmo nome de `package.name` e use o diretório de
saída configurado pela CLI.

### Inspecionar e atualizar

```sh
cppm list --all        # árvore resolvida, incluindo dependências transitivas
cppm why zlib          # app > boost 1.91.0 > zlib 1.3.1
cppm licenses          # bibliotecas agrupadas por licença declarada
cppm outdated          # Current, Wanted (mesma major) e Latest
cppm upgrade           # move todas para Wanted
cppm upgrade sdl --latest
```

`list`, `why` e `licenses` leem o grafo gravado pela última instalação: não
iniciam o Conan nem acessam a rede. Se o manifesto ou o lockfile mudaram desde
então, o comando pede `cppm install` em vez de mostrar uma resolução antiga.
Todos aceitam `--json`. `outdated` consulta o cache local e os remotos Conan
habilitados; com `--offline`, apenas o cache. `upgrade` sem `--latest` nunca
troca a versão major (em versões `0.x`, nunca troca a minor), reinstala o grafo
e só grava manifesto e lockfile se a instalação terminar.

## Bibliotecas compartilhadas entre projetos

As bibliotecas ficam em um único armazenamento por usuário, normalmente
`~/.conan2`. Instalar a mesma biblioteca em outro projeto reutiliza os headers
e binários compatíveis desse armazenamento, sem copiar uma árvore de dependências
para cada projeto. Pacotes fora do catálogo também priorizam versões já salvas;
use `pacote@versão` para pedir outra versão explicitamente. O Conan continua
responsável pela identidade e compatibilidade dos binários.

Depois de uma instalação bem-sucedida, o cppm remove as pastas temporárias de
compilação e os arquivos de download dos pacotes recém-compilados ou baixados.
As fontes e os pacotes prontos permanecem disponíveis para reutilização.
Arquivos idênticos dos pacotes também são compartilhados por conteúdo, inclusive
entre versões e configurações diferentes. O índice BLAKE3 considera o conteúdo
e as permissões de leitura/execução; hard links mantêm uma única cópia dos bytes
no mesmo sistema de arquivos. Os arquivos compartilhados ficam somente leitura.
Quando hard links não estão disponíveis, os arquivos originais são mantidos.

```sh
cppm store path
cppm store status
cppm store status --json
cppm store projects
cppm store dedupe
cppm store prune --dry-run
cppm store prune
cppm store prune --sources
```

Esses comandos funcionam fora de um projeto. Cada instalação concluída registra
o projeto e as revisões exatas de suas dependências diretas, transitivas e de
build. Debug/Release e perfis ficam registrados separadamente. `prune` remove
pacotes gerenciados que não são mais referenciados, além de builds, downloads e
temporários. `--dry-run` mostra os pacotes candidatos sem alterar o armazenamento.
`--sources` remove também fontes baixadas: recompilar uma configuração ausente
pode exigir baixá-los novamente.

Atualizações e remoções liberam referências antigas; projetos excluídos deixam
de prender pacotes na próxima limpeza. Programas iniciados com `cppm exec`
mantêm suas dependências protegidas até terminarem, mesmo após uma atualização.
Metadados inválidos interrompem a coleta. Pacotes desconhecidos, instalados
diretamente pelo Conan, não são apagados; rode `cppm install` nos projetos
existentes para registrá-los. Depois de mover um projeto, rode `cppm install`
no novo local antes da próxima limpeza.

Para escolher outro disco ou pasta, use `--store-dir /caminho/compartilhado`
ou a variável `CPPM_STORE_DIR` em todos os projetos. A prioridade é
`--store-dir`, `CPPM_STORE_DIR`, `CONAN_HOME` e o padrão do Conan. O padrão
continua aproveitando o cache existente; escolher uma pasta vazia cria um cache
separado. Scripts cppm herdam a pasta escolhida, inclusive com caminhos relativos.

`store status` informa o tamanho lógico da pasta home e, para os pacotes
gerenciados, os bytes lógicos, únicos e economizados por compartilhamento.
O total lógico da home conta nomes ligados por hard links; não é uma medição de
blocos físicos. Armazenamento externo do Conan não entra nesse total da home.
Instalações e limpezas cppm no mesmo armazenamento são serializadas por um lock.
Comandos Conan executados diretamente não participam desse lock.

Versões, compiladores, opções e configurações Debug/Release incompatíveis
podem precisar de binários separados. Cada projeto mantém seus próprios
arquivos de build e executáveis em `.cppm/`; a economia é nas dependências
compartilhadas. Builds sem dependências externas continuam funcionando sem Conan.

## VS Code e clangd

Instalações e builds atualizam a configuração CMake e publicam
`compile_commands.json` na raiz do projeto, onde o clangd procura os caminhos de
headers. O VS Code pode estar aberto na pasta do projeto ou em uma pasta pai.
Para atualizar sem compilar a aplicação:

```sh
cppm configure --offline --frozen-lockfile
```

Um `compile_commands.json` personalizado ou um link existente é preservado.
A cópia atual do cppm fica também em `.cppm/compile_commands.json`; a extensão
C/C++ da Microsoft pode usar esse caminho em `C_Cpp.default.compileCommands`.
Se o editor ainda mostrar dados antigos, execute `clangd: Restart language server`.
A configuração do editor é uma etapa posterior à instalação; se o CMake falhar,
as dependências instaladas são mantidas e o cppm indica como repetir essa etapa.

A saída padrão omite a lista de targets CMake/Conan e as mensagens informativas
do OpenCV. Avisos, erros, progresso de compilação e a saída da aplicação continuam
visíveis. Use `cppm exec --verbose` (ou `CPPM_VERBOSE=true`) para mostrar os detalhes.
O mesmo sinalizador funciona com `build`, `install` e `configure` e é herdado por
scripts. Para OpenCV, uma escolha explícita de `OPENCV_LOG_LEVEL` no ambiente,
receita ou perfil tem precedência sobre o padrão WARNING do cppm.

Novos projetos incluem `.vscode/tasks.json`: **Ctrl+Shift+B** compila o projeto;
**Tasks: Run Task → cppm: run** compila e executa com as bibliotecas e o ambiente
de execução do cppm. Para um projeto existente ou quando o VS Code abre sua pasta
pai, crie as tarefas na pasta aberta:

```sh
cppm vscode
cppm vscode --workspace ..
```

Tarefas personalizadas existentes são preservadas. O botão da extensão
“C/C++ Compile Run” continua compilando um arquivo com `g++`; use as tarefas
cppm para projetos com dependências. O banco `compile_commands.json` configura
a análise do editor, enquanto as tarefas fazem o build e a linkagem do projeto.

## Lockfile e perfis

Versione `cppm.toml`, `cppm.lock`, `CMakeLists.txt` e os fontes. Ignore `.cppm/`.
O lockfile guarda as dependências diretas e o grafo de receitas/revisões
resolvido pelo Conan. Ele não fixa o compilador nem torna o build hermético.

```sh
cppm install --frozen-lockfile
cppm build --frozen-lockfile
cppm exec --offline --frozen-lockfile
```

`--frozen-lockfile` exige um lockfile existente e compatível, sem regravá-lo.
`cppm ci` é o atalho para `cppm install --frozen-lockfile` em pipelines.
`--offline` impede consultas aos remotos Conan; receitas, fontes e ferramentas
necessárias ao build precisam estar disponíveis localmente. Uma receita pode
precisar baixar fontes ao compilar um binário ausente, portanto isso não é um
bloqueio de rede para subprocessos.

Na primeira instalação, o perfil `default` é detectado se ainda não existir.
Um perfil existente não é sobrescrito. Para equipes e CI, mantenha um perfil
explícito e use `--profile nome-ou-caminho`. Nesta versão, o mesmo perfil é
aplicado aos contextos host e build: cross-compilation fica fora do escopo.

Debug e Release têm diretórios próprios em `.cppm/build/` e `.cppm/conan/`.
Builds reutilizam uma instalação Conan válida após verificar o perfil, o
lockfile, arquivos gerados e os pacotes locais. Quando nada do que o Conan lê
mudou (executável, home, configuração e arquivos de perfil), essa verificação
não inicia o Conan: um build sem alterações com dependências custa o mesmo que
um sem dependências. Perfis com templates Jinja são sempre resolvidos pelo Conan. Alterações invalidam esse
reaproveitamento automaticamente. A configuração CMake também é reaproveitada;
mudanças de toolchain, ambiente ou configuração exigem um cache novo. O Ninja
acompanha fontes e headers. `cppm install` sempre prepara dependências novamente;
`cppm build --fresh` e `cppm configure --fresh` forçam essa preparação e um novo
cache CMake. `--verbose` mostra o reaproveitamento e os tempos do build.

## Desenvolvimento

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Os testes que usam Conan e rede são separados dos testes locais:

```sh
cargo test --locked --test conan -- --ignored --nocapture
```

Eles verificam fmt, SQLite e uma receita fora do catálogo, incluindo instalação,
descoberta automática, linkagem e reuso offline com lockfile congelado. Os testes
de curl, SDL, SFML e raylib estão separados, pois podem exigir SDKs e bibliotecas
de desenvolvimento instaladas na plataforma:

```sh
cargo test --locked --test libraries -- --ignored --nocapture --test-threads=1
```

Use `CONAN_HOME` para isolar o cache dos testes, se desejar. O ambiente de
execução das DLLs/bibliotecas compartilhadas é exercitado com uma receita local:

```sh
cargo test --locked --test runtime -- --ignored --nocapture
cargo test --locked --test store -- --ignored --nocapture
```

O teste compila uma biblioteca compartilhada real, remove a dependência de RPATH
no Linux e valida `exec` e CTest em Debug/Release, incluindo as variáveis da
receita. O teste `store` usa uma receita local, sem rede: dois projetos apontam
para a mesma biblioteca, compilada uma única vez, e continuam funcionando
offline após a limpeza. Consulte [Windows](docs/windows.md) para repetir os
testes parciais no Wine.

As integrações adicionadas têm verificações próprias com bibliotecas reais:

```sh
cargo test --locked --test databases -- --ignored --nocapture --test-threads=1
cargo test --locked --test libraries standard_ -- --ignored --nocapture --test-threads=1
cargo test --locked --test science -- --ignored --nocapture --test-threads=1
```

Os testes de bancos inicializam clientes, exercitam parsing e validam linkagem;
não exigem servidores de banco. Qt e OpenCV podem compilar muitos componentes
e exigem os requisitos de desenvolvimento da plataforma.

A organização e as decisões de implementação estão em
[docs/architecture.md](docs/architecture.md).

## Limites da versão 0.4

- O binário Windows passou em 13 testes no Wine; MSVC e PowerShell ainda exigem
  execução no runner Windows. macOS ainda não tem validação.
- Uma aplicação principal por projeto; targets adicionais são definidos no CMake.
- Dependências com versões exatas no manifesto; `cppm upgrade` aplica a regra
  de compatibilidade (mesma major), mas faixas SemVer não são gravadas.
- O template WASM usa a biblioteca padrão; dependências Conan para WASM ainda
  precisam de uma integração de cross-compilation própria.
- Sem workspaces, registro ou distribuição próprios de pacotes C++.
- A interface `cppm ui` ainda mostra apenas `[dependencies]`.

O que falta para a paridade com o pnpm está em [docs/roadmap.md](docs/roadmap.md).

