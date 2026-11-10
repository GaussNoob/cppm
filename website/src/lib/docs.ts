import { libraryCatalog, type CatalogGroup } from "./catalog";

export const docGroups = ["Começar", "No projeto", "Bibliotecas", "Avançado"] as const;
export type DocId = "inicio" | "instalacao" | "windows" | "comandos" | "interativo" | "protecao" | "dependencias" | "manifesto" | "builds" | "templates" | "catalogo" | "bancos" | "interfaces" | "numerico" | "protobuf" | "lockfile" | "perfis" | "problemas";
export type DocSection = { id: string; title: string; paragraphs?: readonly string[]; code?: string; filename?: string; table?: { columns: readonly string[]; rows: readonly (readonly string[])[] } };
export type DocTopic = {
  id: DocId; number: string; category: typeof docGroups[number]; title: string; short: string; label: string; description: string; keywords: string;
  code: string; filename: string; paragraphs: readonly string[]; note: string; related: DocId;
  extraCode?: string; extraFilename?: string; sections?: readonly DocSection[]; catalogGroups?: readonly CatalogGroup[]; fullCatalog?: boolean; reference?: string;
};

export const docs: readonly DocTopic[] = [
  {
    id: "inicio", number: "01", category: "Começar", title: "Primeiros passos", short: "Seu primeiro projeto", label: "DO ZERO AO HELLO WORLD",
    description: "Um diretório, alguns comandos e um projeto C++ pronto para ganhar vida.",
    keywords: "iniciar criar init hello world instalação começo exemplo",
    code: "cppm init meu-app\ncd meu-app\ncppm add fmt\ncppm run dev",
    filename: "Terminal",
    paragraphs: ["O init cria o manifesto cppm.toml, o CMakeLists.txt, o arquivo fonte e um teste de execução. O add instala a biblioteca pelo Conan e configura os includes e a linkagem.", "Use cppm doctor antes de começar para conferir as ferramentas disponíveis no PATH."],
    note: "Você precisa de CMake 3.24+, Ninja e GCC, Clang ou MSVC. Projetos com dependências também usam Conan 2.34+.",
    related: "instalacao", reference: "/docs/guide.md",
    sections: [{ id: "estrutura", title: "O que fica no projeto", code: "meu-app/\n  cppm.toml\n  cppm.lock       # gerado na instalação ou build\n  CMakeLists.txt\n  src/main.cpp\n  .cppm/         # arquivos locais, fora do Git", filename: "Estrutura", paragraphs: ["Versione manifesto, lockfile, CMake e fontes. O CMake pertence ao projeto e pode receber novos targets, diretórios e testes."] }],
  },
  {
    id: "instalacao", number: "02", category: "Começar", title: "Instalação", short: "Prepare seu ambiente", label: "AS FERRAMENTAS CERTAS",
    description: "O pacote nativo já inclui a CLI. Rust só é necessário se você preferir compilar o cppm.",
    keywords: "instalar linux windows rust cargo powershell download requisitos cmake ninja conan doctor PATH",
    code: "# Dentro do pacote Linux extraído\nsh install.sh\n\n# Conan para gerenciar dependências\nuv tool install 'conan>=2.34,<3'\n\n# Confira o ambiente\ncppm doctor",
    filename: "Linux · Terminal",
    paragraphs: ["No Linux, o instalador usa ~/.local/bin/cppm e não altera as configurações do shell. Certifique-se de que ~/.local/bin está no PATH.", "No Windows, extraia o ZIP disponível na seção de download e execute o instalador abaixo na pasta extraída. Para seus projetos C++, instale Visual Studio Build Tools com Desktop development with C++, CMake, Ninja e Conan."],
    extraCode: "# Na pasta extraída do ZIP Windows\npowershell -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1\n# Abra um terminal novo após instalar\ncppm doctor",
    extraFilename: "Windows · PowerShell",
    note: "Para compilar a CLI pelo código, use Rust 1.89+. O instalador Windows usa o PATH do usuário e não exige administrador. macOS ainda não tem validação.",
    related: "inicio",
    sections: [{ id: "fonte", title: "Compilar a CLI pelo código", code: "cargo build --release --locked\n# Windows, na raiz do repositório:\npowershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\install.ps1\n# Linux, para instalar diretamente pelo código:\ncargo install --path . --locked", filename: "Com Rust 1.89+ instalado", paragraphs: ["O build da CLI e a compilação dos projetos C++ usam ferramentas distintas. Rust é necessário para compilar o cppm; o pacote pronto inclui o executável."] }],
  },
  {
    id: "windows", number: "16", category: "Começar", title: "Windows", short: "Instalação, MSVC e DLLs", label: "WINDOWS 10/11 · X64",
    description: "Instale a CLI pelo ZIP, prepare o compilador e execute seus projetos com o ambiente das DLLs configurado.",
    keywords: "windows zip exe powershell MSVC visual studio build tools SDK vswhere DLL PATH plugins Qt diagnóstico doctor json",
    code: "# Na pasta extraída do ZIP\npowershell -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1\n\n# Em um terminal novo, com as ferramentas C++ instaladas\ncppm doctor\ncppm init meu-app\ncd .\\meu-app\ncppm install fmt json eigen\ncppm run dev", filename: "PowerShell",
    paragraphs: ["O ZIP inclui cppm.exe, instaladores, licença e os guias. O instalador usa o PATH do usuário e não exige administrador. Rust e MinGW não são necessários para usar o pacote.", "Instale Visual Studio Build Tools com Desktop development with C++, incluindo MSVC e Windows SDK. O cppm detecta o Visual Studio e prepara os subprocessos. Um Developer PowerShell ou um compilador explícito em CC/CXX é respeitado."],
    note: "13 testes de unidade/CLI passaram com o executável Windows no Wine. A execução nativa com MSVC, SDK e PowerShell ainda está pendente; seus testes estão preparados no CI.", related: "comandos", reference: "/docs/windows.md",
    sections: [
      { id: "ferramentas", title: "CMake, Ninja e Conan", code: 'py -m pip install --user "conan>=2.34,<3" "cmake>=3.24" ninja', filename: "Com Python 3 instalado", paragraphs: ["As ferramentas precisam estar no PATH do terminal. O guia Windows para download explica como localizar a pasta de scripts da instalação Python."] },
      { id: "dlls", title: "Bibliotecas compartilhadas", paragraphs: ["exec e test carregam o ambiente Conan da configuração escolhida: caminhos das DLLs e variáveis como QT_PLUGIN_PATH. Isso também vale para os scripts padrão dev e test. Debug e Release usam ambientes separados.", "Para distribuir sua aplicação a outra máquina, empacote as DLLs, plugins e recursos exigidos pelas bibliotecas. O ZIP do cppm distribui a CLI."] },
      { id: "diagnostico", title: "Teste o ambiente antes de começar", code: "cppm doctor\ncppm doctor --json > diagnostico.json", filename: "PowerShell", paragraphs: ["O doctor verifica versões e compila e executa um programa C++ temporário. O relatório inclui todos os problemas encontrados, com orientações para resolver cada um. Ele não instala ferramentas nem consulta a rede."] },
    ],
  },
  {
    id: "comandos", number: "03", category: "No projeto", title: "Comandos", short: "CLI, scripts e argumentos", label: "COMANDOS E SCRIPTS",
    description: "Crie, instale, compile e execute. Os comandos seguem o mesmo fluxo em cada projeto.",
    keywords: "cli comandos init add install remove run exec packages doctor ajuda help diretório",
    code: "cppm ui                    # Abre a interface interativa\ncppm init meu-app          # Cria um projeto\ncppm add fmt json          # Adiciona bibliotecas juntas\ncppm install               # Instala as dependências\ncppm remove fmt            # Remove uma dependência\ncppm run                   # Lista os scripts\ncppm run dev               # Compila em Debug e executa\ncppm run build             # Compila em Release\ncppm build obf             # Compila com ofuscação LLVM\ncppm test obf              # Testa o build ofuscado\ncppm run test              # Executa os testes\ncppm packages              # Lista o catálogo\ncppm doctor                # Testa ferramentas e compilador\ncppm doctor --json         # Diagnóstico em JSON",
    filename: "Referência rápida",
    paragraphs: ["Use -C pasta para escolher o diretório de trabalho. Dentro do projeto, o cppm também encontra o manifesto a partir de subdiretórios.", "Argumentos depois de -- são repassados à aplicação ou ao script. O código de saída do processo chega ao seu terminal e ao CI."],
    extraCode: "cppm -C projetos/meu-app run dev\ncppm run dev -- \"um argumento com espaços\"",
    extraFilename: "Diretórios e argumentos",
    note: "Consulte cppm --help ou cppm <comando> --help para ver as opções completas de cada comando.",
    related: "builds", reference: "/docs/guide.md",
    sections: [{ id: "scripts", title: "Scripts portáveis", code: '[scripts]\ndev = ["cppm", "exec", "--"]\nbuild = ["cppm", "build", "--release"]\ntest = ["cppm", "test", "--"]', filename: "cppm.toml", paragraphs: ["Arrays executam diretamente no Linux e no Windows, sem shell. Strings usam sh no Linux e Windows PowerShell no Windows. O código de saída é preservado.", "Os scripts partem da raiz do projeto. Perfil, offline e lockfile são herdados por comandos cppm chamados por eles."] }],
  },
  {
    id: "dependencias", number: "04", category: "Bibliotecas", title: "Dependências", short: "Instalar, versionar e remover", label: "CONECTADO AO CONAN 2",
    description: "Use bibliotecas do Conan com os includes e os targets CMake configurados pelos metadados da receita.",
    keywords: "pacotes dependências bibliotecas fmt json nlohmann sqlite curl sdl sfml raylib spdlog conan versões lockfile frozen offline",
    code: "cppm add fmt spdlog json\ncppm install sqlite eigen glm\ncppm add fmt@11.0.2",
    filename: "Adicionar bibliotecas",
    paragraphs: ["json é um alias de nlohmann_json, sqlite de sqlite3 e curl de libcurl. Sem versão explícita, pacotes do catálogo usam uma versão padrão; outras receitas consultam uma versão estável no Conan. O manifesto sempre recebe a versão exata.", "add e install aceitam vários pacotes em uma operação. Aliases duplicados são agrupados, versões conflitantes são rejeitadas e uma falha preserva o manifesto e o lockfile anteriores.", "Versione cppm.lock para compartilhar o grafo resolvido. O lockfile registra receitas e revisões; ele não fixa o compilador nem torna o build hermético."],
    extraCode: "cppm install --frozen-lockfile\ncppm exec --offline --frozen-lockfile",
    extraFilename: "Lockfile e cache local",
    note: "--offline impede consultas aos remotos Conan. Uma receita ainda pode baixar fontes ao compilar; não é um bloqueio de rede para subprocessos. Bibliotecas gráficas podem exigir SDKs da plataforma.",
    related: "catalogo", reference: "/docs/packages.md",
    sections: [{ id: "overrides", title: "Mapeamento CMake explícito", code: "cppm add pacote@1.2.3 --cmake-package Pacote --target Pacote::Pacote\ncppm remove pacote", filename: "Terminal", paragraphs: ["Os metadados da receita configuram includes e linkagem. Use o override quando uma receita precisar de nomes específicos. add e install nome adicionam dependências; install sem nome instala o manifesto existente.", "Se o Conan detectar versões transitivas incompatíveis, escolha versões ou opções compatíveis. O lockfile não resolve um conflito de versões."] }],
  },
  {
    id: "manifesto", number: "05", category: "No projeto", title: "cppm.toml", short: "Estrutura, fontes e configuração", label: "PEQUENO E LEGÍVEL",
    description: "Nome, padrão C++, dependências e scripts em um arquivo TOML que cabe em uma tela.",
    keywords: "manifesto toml configuração scripts arrays padrão standard c++20 package cmake targets",
    code: '[package]\nname = "meu-app"\nversion = "0.1.0"\ncpp-standard = 20\n\n[dependencies]\nfmt = "11.0.2"\n\n[scripts]\ndev = ["cppm", "exec", "--"]\nbuild = ["cppm", "build", "--release"]\ntest = ["cppm", "test", "--"]',
    filename: "cppm.toml",
    paragraphs: ["Scripts em arrays são executados diretamente, sem shell, no Linux e no Windows. Os scripts sempre partem da raiz do projeto.", "O CMakeLists.txt é seu. Depois do init, a CLI não o reescreve: adicione diretórios, testes, targets e opções conforme o projeto crescer."],
    note: "Versione cppm.toml, cppm.lock, CMakeLists.txt e seus fontes. Adicione .cppm/ ao .gitignore.",
    related: "builds",
  },
  {
    id: "builds", number: "06", category: "No projeto", title: "Builds e testes", short: "Debug, Release e CTest", label: "CMAKE + NINJA, POR BAIXO",
    description: "Debug para desenvolver, Release para distribuir e CTest para verificar o que você construiu.",
    keywords: "build compilação ninja cmake release debug ctest testes jobs paralelo profile perfil ci frozen cache",
    code: "cppm run dev\ncppm run build\ncppm run test\n\n# Build com quatro tarefas em paralelo\ncppm build --release -j 4\n\n# Execute um teste específico\ncppm test -- -R app_smoke",
    filename: "Compilar e testar",
    paragraphs: ["Debug e Release usam diretórios próprios em .cppm/build/. O Conan reutiliza seu cache compartilhado e o Ninja reaproveita objetos que não mudaram.", "exec e test aplicam o ambiente de execução das dependências Conan aos subprocessos. Os caminhos das DLLs e dos plugins Qt são carregados para a configuração escolhida.", "Para equipes e CI, use --profile nome-ou-caminho e --frozen-lockfile. Nesta versão, o mesmo perfil é aplicado aos contextos host e build."],
    extraCode: "cppm build --release --profile meu-perfil --frozen-lockfile",
    extraFilename: "Um perfil explícito no CI",
    note: "O cppm trabalha com uma aplicação principal por projeto. Targets adicionais são definidos no CMake. Watch, workspaces e cross-compilation ainda estão fora do escopo.",
    related: "inicio",
  },
  {
    id: "templates", number: "07", category: "No projeto", title: "Templates", short: "App, API HTTP e WebAssembly", label: "ESCOLHA UM PONTO DE PARTIDA",
    description: "Comece com uma aplicação nativa, uma API Drogon ou um módulo WebAssembly.", keywords: "template drogon api http wasm webassembly emscripten node python servidor porta serve",
    code: "cppm init meu-app\ncppm init minha-api --template drogon\ncppm init meu-wasm --template wasm", filename: "Terminal",
    paragraphs: ["O template padrão cria manifesto, CMake, fonte e teste de execução. Drogon inclui a dependência, rotas / e /health, respostas JSON e um teste HTTP.", "No WASM, ative o SDK Emscripten. run dev gera HTML, JS e WASM e serve o resultado localmente. run test executa o módulo com Node.js."],
    note: "Python 3 é usado pelo servidor WASM e pelo teste HTTP Drogon. Bibliotecas Conan nativas não podem ser usadas no target WASM nesta versão.", related: "dependencias", reference: "/docs/guide.md",
    sections: [{ id: "portas", title: "Portas de desenvolvimento", code: "cppm -C minha-api run dev -- --port 9090\ncppm -C meu-wasm run dev -- --port 9090", filename: "Escolher outra porta" }],
  },
  {
    id: "catalogo", number: "08", category: "Bibliotecas", title: "Catálogo", short: `${libraryCatalog.length} pacotes por categoria`, label: "UMA REFERÊNCIA PARA ESCOLHER",
    description: "Comandos para instalar, versões padrão, headers e targets CMake. O catálogo abaixo vem do próprio cppm.", keywords: "catálogo bibliotecas boost httplib asio imgui qt catch2 googletest opencv eigen protobuf glm onetbb fmt spdlog zlib json",
    code: "cppm packages\ncppm install nome\ncppm add nome@versao", filename: "Consultar e instalar", paragraphs: ["Instale uma biblioteca por comando, conforme a necessidade do projeto. Os nomes são normalizados: Boost, OpenCV, GLM e Qt também funcionam.", "As versões são padrões fixos para começar. Não representam uma promessa de usar o lançamento mais recente; pacote@versão escolhe outra receita publicada."],
    note: "Cada biblioteca mantém sua licença, suas opções e seus requisitos de plataforma. O Conan verifica o grafo de dependências.", related: "bancos", fullCatalog: true, reference: "/docs/packages.md",
  },
  {
    id: "bancos", number: "09", category: "Bibliotecas", title: "Bancos de dados", short: "SQL, Redis e MongoDB", label: "DRIVERS PRONTOS PARA O PROJETO",
    description: "Instale os clientes de SQLite, MySQL/MariaDB, PostgreSQL, Redis e MongoDB e use os headers diretamente no C++.", keywords: "banco dados database sql sqlite sqlite3 mysql mariadb postgres postgresql postgress libpq redis hiredis mongodb mongo bson conexão",
    code: "cppm install sqlite\ncppm install mysql\ncppm install postgres\ncppm install redis\ncppm install mongodb", filename: "Escolha o banco do projeto",
    paragraphs: ["MySQL e MariaDB usam o mesmo MariaDB Connector/C, compatível com os dois servidores. mysql e mariadb mantêm uma única dependência direta. postgres, postgresql e postgress selecionam libpq; mongodb e mongo selecionam mongo-c-driver.", "SQLite pode operar em um arquivo local ou em memória. Os demais drivers conectam a servidores configurados separadamente. A versão após @ é a do cliente, não a do servidor."],
    note: "O cppm instala o driver. Host, porta, autenticação e criação do banco são configurados no servidor e na aplicação.", related: "interfaces", catalogGroups: ["Bancos de dados"], reference: "/docs/packages.md",
    sections: [{ id: "postgres", title: "PostgreSQL: configuração da conexão", paragraphs: ["libpq aceita PGHOST, PGPORT, PGDATABASE e PGUSER. O exemplo abaixo inicializa uma conexão com a configuração do ambiente e libera os recursos."], code: '#include <libpq-fe.h>\n\nint main() {\n    PGconn* connection = PQconnectdb("");\n    const bool ok = connection && PQstatus(connection) == CONNECTION_OK;\n    if (connection) PQfinish(connection);\n    return ok ? 0 : 1;\n}', filename: "src/main.cpp" }, { id: "validacao", title: "O que os exemplos verificam", paragraphs: ["examples/databases contém programas que inicializam clientes, validam BSON e fazem parsing do protocolo Redis sem depender de servidores em execução. Conexões e autenticação precisam de um teste com o seu servidor."] }],
  },
  {
    id: "interfaces", number: "10", category: "Bibliotecas", title: "Qt e Dear ImGui", short: "Biblioteca, janela e renderização", label: "INTERFACES NATIVAS",
    description: "Escolha a interface adequada ao projeto e configure os recursos de janela, eventos e renderização que ela precisa.", keywords: "gui gráfico interface qt qt6 imgui dear-imgui dearimgui widget opengl glfw sdl backend moc q_object",
    code: "cppm install imgui\n# ou, para um projeto Qt:\ncppm install qt", filename: "Terminal", paragraphs: ["Dear ImGui fornece widgets de interface imediata. Para exibi-los, configure um backend de plataforma e renderização, como SDL + OpenGL ou GLFW + OpenGL, e o loop de eventos.", "Qt disponibiliza Core, Gui e Widgets conforme as opções da receita. O CMake é seu: configure a automação necessária para Q_OBJECT, formulários .ui e recursos .qrc."],
    note: "Qt usa bibliotecas dinâmicas e desativa seu plugin PostgreSQL no padrão devido à receita Conan. libpq pode ser usado diretamente. A distribuição da aplicação deve incluir as bibliotecas e plugins Qt necessários.", related: "numerico", catalogGroups: ["Interfaces"], reference: "/docs/packages.md",
    sections: [{ id: "qt-core", title: "Verificar Qt Core", code: '#include <QCoreApplication>\n#include <QString>\n\nint main(int argc, char** argv) {\n    QCoreApplication app(argc, argv);\n    return QString::number(42) == QStringLiteral("42") ? 0 : 1;\n}', filename: "src/main.cpp", paragraphs: ["No Linux, o pacote Qt é compilado com o dialeto GNU do padrão declarado, habilitando as extensões de inteiros do framework. A aplicação continua usando o padrão ISO do manifesto."] }],
  },
  {
    id: "numerico", number: "11", category: "Bibliotecas", title: "Imagens e matemática", short: "OpenCV, Eigen, GLM e oneTBB", label: "DO DADO AO ALGORITMO",
    description: "Visão computacional, álgebra linear, matemática gráfica e trabalho em paralelo no mesmo fluxo do cppm.", keywords: "opencv eigen glm onetbb tbb imagem visão matriz vetor matemático paralelismo threading desempenho",
    code: "cppm install opencv\ncppm install eigen\ncppm install glm\ncppm install onetbb", filename: "Escolha as bibliotecas", paragraphs: ["OpenCV trabalha com imagens e visão computacional. Eigen fornece matrizes e álgebra linear; GLM é voltada à matemática de aplicações gráficas; oneTBB fornece algoritmos e tarefas paralelas.", "Eigen 3.4.0 é o padrão para coexistir com a versão exigida por OpenCV 4.14.0. Uma versão diferente pode ser selecionada com @, desde que o restante do grafo seja compatível."],
    note: "tbb e one-tbb são aliases de onetbb. O OpenCV começa com FFmpeg desativado. Para vídeo por FFmpeg, defina opencv/*:with_ffmpeg=True em [options] no perfil e use o mesmo perfil ao instalar e compilar.", related: "protobuf", catalogGroups: ["Matemática e visão"], reference: "/docs/packages.md",
    sections: [{ id: "parallel-for", title: "Executar um trabalho em paralelo", code: "#include <oneapi/tbb/parallel_for.h>\n#include <atomic>\n\nint main() {\n    std::atomic<int> count{0};\n    oneapi::tbb::parallel_for(0, 42, [&](int) { ++count; });\n    return count == 42 ? 0 : 1;\n}", filename: "src/main.cpp" }],
  },
  {
    id: "protobuf", number: "12", category: "Bibliotecas", title: "JSON e Protobuf", short: "Serialização e schemas", label: "DADOS NA APLICAÇÃO",
    description: "Use JSON para documentos flexíveis e Protobuf para mensagens com tipos definidos por um schema.", keywords: "serialização json nlohmann protobuf protocol buffers schema proto protoc gerar mensagens parsing",
    code: "cppm install json\ncppm install protobuf", filename: "Terminal", paragraphs: ["json seleciona nlohmann_json e usa <nlohmann/json.hpp>. protobuf instala o runtime Protocol Buffers e seus recursos de integração CMake.", "Para mensagens próprias, crie um schema .proto e acrescente a geração ao CMake. Instalar o pacote não gera os fontes automaticamente; o código gerado e o runtime precisam de versões compatíveis."],
    note: "protocol-buffers é alias de protobuf. O projeto mantém o controle dos fontes gerados, includes e comandos de geração.", related: "lockfile", catalogGroups: ["Serialização"], reference: "/docs/packages.md",
    sections: [{ id: "schema", title: "Um schema Protobuf", code: 'syntax = "proto3";\n\nmessage Pessoa {\n  string nome = 1;\n  int32 idade = 2;\n}', filename: "proto/pessoa.proto", paragraphs: ["A etapa de geração produz os arquivos C++ usados pelo runtime. Mantenha essa etapa no CMake junto dos seus targets."] }, { id: "geracao", title: "Gerar C++ no CMake", code: 'add_custom_command(\n    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.cc"\n           "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.h"\n    COMMAND protobuf::protoc\n    ARGS "--cpp_out=${CMAKE_CURRENT_BINARY_DIR}"\n         "--proto_path=${CMAKE_CURRENT_SOURCE_DIR}/proto"\n         "${CMAKE_CURRENT_SOURCE_DIR}/proto/pessoa.proto"\n    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/proto/pessoa.proto"\n    VERBATIM)\ntarget_sources(meu-app PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/pessoa.pb.cc")\ntarget_include_directories(meu-app PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")', filename: "Adicionar ao CMakeLists.txt", paragraphs: ["Use o nome do target executável do seu projeto no lugar de meu-app. Depois, inclua <pessoa.pb.h> no código e execute cppm run dev.", "O cppm aponta protobuf::protoc para o compilador do mesmo pacote do runtime, evitando um protoc incompatível previamente instalado no PATH."] }],
  },
  {
    id: "lockfile", number: "13", category: "Avançado", title: "Lockfile e offline", short: "Reproduzir o grafo de dependências", label: "DEPENDÊNCIAS REGISTRADAS",
    description: "Compartilhe as receitas resolvidas e confira o cache antes de trabalhar sem consultar remotos.", keywords: "cppm.lock lockfile frozen offline revisão recipe cache reproduzir hermético versão",
    code: "cppm install\ncppm install --frozen-lockfile\ncppm exec --offline --frozen-lockfile", filename: "Terminal", paragraphs: ["Versione cppm.lock junto de cppm.toml. --frozen-lockfile exige que os arquivos sejam compatíveis e não regrava o lockfile. Para atualizar após uma alteração, execute install sem essa opção.", "--offline impede consultas aos remotos Conan. Receitas, ferramentas e fontes precisam estar disponíveis; uma receita ainda pode baixar fontes durante um build. Não é um bloqueio de rede de subprocessos."],
    note: "O lockfile registra receitas e revisões. Ele não fixa o compilador nem torna o build hermético.", related: "perfis", reference: "/docs/guide.md",
    sections: [{ id: "cache", title: "Arquivos do projeto e do ambiente", table: { columns: ["Versionar", "Manter local"], rows: [["cppm.toml e cppm.lock", ".cppm/"], ["CMakeLists.txt e fontes", "Cache compartilhado do Conan"], ["Perfis da equipe, sem credenciais", "Toolchains e SDKs instalados"]] } }],
  },
  {
    id: "perfis", number: "14", category: "Avançado", title: "Perfis e CI", short: "Opções e ferramentas explícitas", label: "O AMBIENTE TAMBÉM IMPORTA",
    description: "Defina compilador, opções de bibliotecas e tarefas paralelas em um perfil do Conan compartilhado pela equipe.", keywords: "perfil profile ci pipeline compilador gcc clang msvc opções openssl tls jobs conan conf settings",
    code: "include(default)\n\n[conf]\ntools.build:jobs=4\n\n[options]\ncpp-httplib/*:with_openssl=True", filename: "profiles/dev",
    paragraphs: ["Use o mesmo perfil ao instalar e compilar. Na primeira execução sem perfil explícito, o default é detectado apenas se estiver ausente; perfis existentes não são sobrescritos.", "Nesta versão, um único perfil é aplicado a host e build. Cross-compilation ainda está fora do escopo."],
    extraCode: "cppm install cpp-httplib --profile ./profiles/dev\ncppm run dev --profile ./profiles/dev\ncppm build --release --profile ./profiles/dev --frozen-lockfile\ncppm test --profile ./profiles/dev --frozen-lockfile", extraFilename: "Terminal e CI",
    note: "Não inclua credenciais em perfis versionados. A opção de tarefas do Conan controla a compilação das dependências; -j controla o build da aplicação.", related: "problemas", reference: "/docs/guide.md",
  },
  {
    id: "problemas", number: "15", category: "Avançado", title: "Problemas comuns", short: "Diagnóstico e próximos passos", label: "QUANDO ALGO NÃO COMPILA",
    description: "Confira ferramentas, versão, perfil e a primeira mensagem de erro antes de alterar a integração do projeto.", keywords: "erro falha ajuda troubleshooting PATH conan cmake ninja compiler toolchain gssapi kerberos banco conexão recusada conflito versões",
    code: "cppm --version\ncppm doctor\ncppm doctor --json\ncppm packages\ncppm install", filename: "Primeiras verificações", paragraphs: ["O doctor verifica as versões de CMake, Ninja e Conan e compila e executa um programa C++ temporário. Todos os problemas encontrados são apresentados; use --json para salvar o relatório. Não há instalação automática nem consultas à rede.", "A CLI preserva o status de saída da ferramenta que falhou. A primeira mensagem de Conan, CMake ou do compilador costuma indicar a causa. Uma instalação malsucedida não altera o manifesto nem o lockfile versionado."],
    note: "O executável Windows passou em testes no Wine. MSVC e PowerShell ainda exigem execução no runner Windows. macOS ainda não tem validação.", related: "inicio", reference: "/docs/guide.md",
    sections: [{ id: "diagnostico", title: "Sintoma e verificação", table: { columns: ["Sintoma", "O que conferir"], rows: [["cppm não encontrado", "PATH do usuário; no Windows, abra um terminal novo após instalar."], ["Conan não encontrado", "Instalação por uv ou pipx e PATH disponível ao terminal."], ["Não há manifesto", "Entre no projeto ou use -C caminho."], ["Lockfile incompatível", "Execute cppm install sem --frozen-lockfile e versione o resultado."], ["Pacote ausente offline", "Instale online com o mesmo perfil e configuração."], ["Conflito de versões", "Escolha versões ou opções compatíveis da dependência comum."], ["Receita pede SDK do sistema", "Instale os requisitos de desenvolvimento informados pela receita."], ["Executável não encontrado", "O target precisa ter o nome de package.name."], ["Banco recusa conexão", "Servidor, host, porta e autenticação são configurados separadamente."], ["ImGui não abre janela", "Configure os backends de janela/renderização e o loop de eventos."]] } }, { id: "ajustes", title: "Ajustes de receitas no Linux", paragraphs: ["O cppm completa a linkagem de GSSAPI do libpq 17.11 estático quando a receita detecta Kerberos no sistema. O ajuste pode coexistir com o de libdecor em SDL 3.4 com Wayland."] }],
  },
  {
    id: "interativo", number: "17", category: "No projeto", title: "Interface interativa", short: "Explore, selecione e execute", label: "TUDO NO SEU TERMINAL",
    description: "Bibliotecas, dependências e comandos em uma interface de terminal com pesquisa e navegação pelo teclado.",
    keywords: "ui tui interactive interface interativa terminal painel busca pesquisa catalogo categorias atalho paleta selecionar lote",
    code: "cppm ui\n# Também pode abrir outro projeto\ncppm -C ./meu-app ui\n# Ou consultar apenas o cache local\ncppm --offline ui", filename: "Abra a interface",
    paragraphs: ["Use as quatro abas para acompanhar o projeto, explorar bibliotecas, conferir dependências e executar scripts ou ações prontas. O catálogo consulta os remotos Conan habilitados em segundo plano, incluindo receitas sem versão padrão no cppm.", "Pesquise por nomes, aliases ou assuntos. Espaço seleciona várias bibliotecas e Enter abre a prévia para instalar o conjunto. A tecla a aceita nomes e versões como json sdl3 fmt@11.0.2."],
    note: "Cada ação mostra o comando antes de executar. A saída aparece no terminal normal; Enter retorna ao painel. Requer um terminal com pelo menos 52 × 19. Aliases: cppm tui e cppm interactive.", related: "protecao", reference: "/docs/guide.md",
    sections: [
      { id: "atalhos", title: "Atalhos que você não precisa decorar", table: { columns: ["Tecla", "Ação"], rows: [["Tab / 1–4", "Trocar de aba"], ["/", "Pesquisar na lista"], ["↑↓ / j k", "Navegar"], ["Espaço / Enter", "Selecionar bibliotecas / abrir uma ação"], ["c / f", "Alternar categoria / filtro do catálogo"], ["a / x", "Adicionar nome ou versão / limpar a seleção"], ["Ctrl+P", "Abrir a paleta de comandos"], ["r / ? / q", "Atualizar / ajuda / sair"]] } },
      { id: "dependencias", title: "O manifesto e o lockfile à vista", paragraphs: ["A aba Dependências mostra versões e a correspondência com o lockfile. Enter sincroniza o manifesto e d abre a prévia de remoção. O estado No lockfile não afirma que todos os perfis já têm binários preparados.", "Fora de um projeto, você pode explorar bibliotecas e verificar o ambiente. A aba Comandos oferece a criação de um projeto na pasta atual. Falhas de consulta ao Conan preservam o catálogo anterior."] },
    ],
  },
  {
    id: "protecao", number: "18", category: "Avançado", title: "Build ofuscado", short: "Ofuscador LLVM incluído", label: "UMA CAMADA A MAIS NA DISTRIBUIÇÃO",
    description: "Gere um Release separado com passes LLVM para dificultar a leitura estática de código e literais elegíveis.",
    keywords: "obf ofuscacao ofuscação plugin llvm clang proteger protecao codigo strings flattening substituicao compilação binario símbolos metadados",
    code: "cppm build obf\ncppm exec obf -- argumento\ncppm test obf", filename: "Plugin preparado automaticamente",
    paragraphs: ["O cppm inclui os fontes de uma revisão fixa do Obfuscator-LLVM. No primeiro uso, compila o plugin contra o SDK LLVM instalado e guarda o resultado em .cppm/tools/. Os próximos builds reutilizam esse cache; o plugin não precisa ser baixado à parte.", "O preset ativa mascaramento de strings locais elegíveis, achatamento de fluxo de controle, substituição de instruções e divisão de blocos. Também reduz símbolos, remove informações de depuração e remapeia caminhos do projeto. A saída fica em .cppm/build/obf/bin/."],
    note: "Requer Clang e um SDK de desenvolvimento LLVM 18–23 correspondente, com llvm-config. Validado em Linux com LLVM 23.1.4. O plugin incluído não é suportado pelo host MSVC/Windows. Ofuscação dificulta a análise; não torna um binário impossível de reverter nem protege segredos recuperáveis pela aplicação.", related: "interativo", reference: "/docs/guide.md",
    sections: [
      { id: "ambiente", title: "O SDK escolhe o compilador", code: "# Se houver mais de um SDK instalado\nCPPM_LLVM_CONFIG=/caminho/llvm-config cppm build obf", filename: "Linux · Terminal", paragraphs: ["Sem CC/CXX explícitos, o cppm seleciona o Clang do SDK. Se essas variáveis estiverem definidas, os compiladores precisam corresponder à versão do llvm-config escolhido. SDK ausente ou incompatível causa erro; o comando não troca silenciosamente para proteção básica.", "Com dependências e perfil default, cria um perfil Clang local que inclui o default, preservando o arquivo original. Um perfil explícito deve selecionar o Clang compatível. Dependências usam Release e não recebem os passes de ofuscação."] },
      { id: "basico", title: "Reduzir metadados sem SDK LLVM", code: "cppm build obf --obf-basic\ncppm test obf --obf-basic", filename: "GCC, Clang ou MSVC", paragraphs: ["O modo básico mantém LTO, redução de símbolos e remapeamento de caminhos com o compilador normal. Dispensa o plugin e o SDK LLVM; não transforma o fluxo de controle nem mascara strings. Também está disponível na paleta da interface interativa."] },
      { id: "externo", title: "Continuar usando um plugin próprio", code: "CXX=clang++ cppm build obf --obf-plugin /caminho/plugin.so", filename: "Plugin externo compatível", paragraphs: ["Um plugin externo substitui o incluído. Ele deve registrar passes no pipeline automático do Clang e ser compatível com sua versão/ABI. As transformações e a configuração ficam sob responsabilidade desse plugin."] },
      { id: "escopo", title: "Teste o que você vai distribuir", paragraphs: ["A proteção atua no executável principal, cujo target deve corresponder a package.name. Outros targets, recursos e bibliotecas externas não são ofuscados automaticamente. Strings ODR compartilhadas, TLS e construções não elegíveis podem permanecer legíveis.", "Use cppm test obf e teste seu fluxo real. Distribua o executável final com as bibliotecas e os recursos necessários; fontes, caches, objetos e símbolos de desenvolvimento ficam fora do pacote da sua aplicação. O preset não suporta WebAssembly."] },
    ],
  },
];

export function docSearchText(doc: DocTopic) {
  const catalog = doc.fullCatalog || doc.catalogGroups ? libraryCatalog.filter((library) => doc.fullCatalog || doc.catalogGroups?.includes(library.group)).map((library) => `${library.label} ${library.name} ${library.version} ${library.command} ${library.header} ${library.aliases.join(" ")}`).join(" ") : "";
  return `${doc.title} ${doc.short} ${doc.description} ${doc.keywords} ${doc.code} ${doc.paragraphs.join(" ")} ${doc.extraCode ?? ""} ${doc.note} ${doc.sections?.map((section) => `${section.title} ${section.code ?? ""} ${section.paragraphs?.join(" ") ?? ""} ${section.table?.rows.flat().join(" ") ?? ""}`).join(" ") ?? ""} ${catalog}`.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

export function openDoc(id: DocId) {
  window.dispatchEvent(new CustomEvent("cppm:open-doc", { detail: id }));
  const target = document.getElementById("documentacao");
  target?.scrollIntoView({ behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth", block: "start" });
}
