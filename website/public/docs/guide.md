# Guia do cppm

## Navegação

1. [Instalação e requisitos](installation.md)
2. [Diagnóstico do ambiente](#diagnóstico-do-ambiente)
3. [Primeiro projeto](#primeiro-projeto)
4. [Estrutura e manifesto](#estrutura-e-manifesto)
5. [Scripts e argumentos](#scripts-e-argumentos)
6. [Instalar várias bibliotecas](#instalar-várias-bibliotecas)
7. [Builds e testes](#builds-e-testes)
8. [Bibliotecas e bancos de dados](packages.md)
9. [Templates](#templates)
10. [Lockfile, offline e perfis](#lockfile-offline-e-perfis)
11. [Problemas comuns](#problemas-comuns)
12. [Arquitetura](architecture.md)
13. [Guia Windows](windows.md)
14. [Armazenamento compartilhado](#armazenamento-compartilhado)
15. [Interface interativa](#interface-interativa)
16. [Build protegido e plugins LLVM](#build-protegido-e-plugins-llvm)

## Interface interativa

Execute `cppm ui` dentro de um projeto ou `cppm -C pasta ui` de qualquer local.
Os aliases `cppm tui` e `cppm interactive` abrem a mesma interface. Ela oferece
quatro abas: visão geral, catálogo pesquisável, dependências e comandos prontos.

| Tecla | Ação |
| --- | --- |
| `Tab` / `Shift+Tab`, `←` / `→`, `1`–`4` | Trocar de aba |
| `↑` / `↓`, `j` / `k` | Navegar na lista |
| `PgUp` / `PgDn`, `Home` / `End` | Avançar na lista, ir ao início/fim |
| `/`, `Enter`, `Esc` | Editar busca, concluir edição, limpar busca |
| `Ctrl+P` | Abrir e pesquisar comandos |
| `Espaço` | Marcar/desmarcar biblioteca no catálogo |
| `Enter` / `i` | Instalar a seleção ou abrir a ação atual |
| `a` | Adicionar nomes ou versões à seleção, como `json sdl3 fmt@11.0.2` |
| `c`, `f` | Alternar categoria e filtro do catálogo |
| `x` | Limpar seleção de bibliotecas |
| `r` | Atualizar catálogo Conan ou reler o projeto, conforme a aba |
| `d` | Remover a dependência selecionada, com prévia |
| `?` / `F1` | Ajuda de atalhos |
| `q` / `Ctrl+C` | Sair |

A aba Explorar consulta os remotos Conan habilitados em segundo plano e combina
as receitas com as sugestões locais. A busca considera nomes, aliases,
categorias e descrições disponíveis. O filtro **Sem padrão cppm** destaca
bibliotecas cuja versão e integração CMake serão resolvidas pelo Conan;
**Fora do projeto** mostra o que ainda não foi declarado. `a` também permite
escolher outra versão de uma biblioteca já declarada.

Com `cppm --offline ui`, a descoberta usa somente o cache local. Se Conan
estiver ausente, a consulta falhar ou demorar mais de 45 segundos, a interface
permanece disponível com o catálogo anterior; `r` tenta novamente. A descoberta
considera receitas `nome/versão`, sem referências `@usuário/canal`. As sugestões
locais não representam uma verificação de compatibilidade com todos os perfis.

Na aba Dependências, **No lockfile** significa que a declaração corresponde à
resolução salva; **Pendente de sincronização** indica uma diferença ou lockfile
ausente/inválido. Esses estados não afirmam que há um binário preparado para
todo perfil. `Enter` instala o manifesto no ambiente atual.

Antes de executar uma ação, revise a prévia e confirme com `Enter` ou cancele
com `Esc`. Setas permitem rolar prévias longas. A saída do comando aparece no
terminal normal; pressione `Enter` ao final para retornar à interface. Uma falha
é exibida e permite continuar navegando. A seleção de instalação é preservada
se o comando falhar. Os comandos usam as mesmas validações, locks, scripts e
opções globais da CLI, incluindo `--store-dir`, `--profile`, `--offline` e
`--frozen-lockfile`. A limpeza do armazenamento oferecida no menu é uma
simulação (`--dry-run`).

Fora de um projeto, é possível explorar bibliotecas, verificar o ambiente e
criar um aplicativo C++ na pasta atual pela aba Comandos. A interface requer
entrada e saída conectadas a um terminal; pipes e redirecionamentos recebem
uma mensagem de erro. Em terminais menores que 52 × 19, amplie a janela para
retomar a navegação.

## Diagnóstico do ambiente

`cppm doctor` informa versões das ferramentas e compila e executa um programa
C++ temporário com CMake/Ninja/CTest. Dentro de um projeto, usa seu padrão C++;
fora de um projeto, usa C++20. Ele não instala ferramentas nem acessa a rede.
Use `cppm doctor --json` para salvar um relatório completo. Todos os problemas
encontrados são apresentados e qualquer falha retorna status diferente de zero.
No Windows, o diagnóstico também exercita a descoberta do ambiente MSVC.

## Primeiro projeto

Com CMake, Ninja, compilador C++ e Conan disponíveis:

```sh
cppm doctor
cppm init meu-app
cd meu-app
cppm add fmt
cppm run dev
```

`init` não sobrescreve arquivos do usuário. Um projeto sem dependências
externas pode ser compilado sem Conan. Use `cppm --help` e
`cppm <comando> --help` para consultar as opções.

## Estrutura e manifesto

```text
meu-app/
  cppm.toml
  cppm.lock          # gerado na instalação/build
  CMakeLists.txt
  src/main.cpp
  .cppm/            # arquivos locais; não versionar
```

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

O CMake pertence ao projeto e não é reescrito depois de `init`. Adicione targets,
testes, fontes geradas e opções nele. `exec` espera um executável com o nome de
`package.name`. O padrão é C++20; o manifesto aceita 11, 14, 17, 20, 23 e 26,
desde que o compilador e as dependências suportem o padrão escolhido.

## Scripts e argumentos

Arrays executam diretamente, sem shell, no Linux e no Windows. Strings usam
`sh` no Linux e Windows PowerShell no Windows. Os scripts partem da raiz do
projeto, mesmo quando chamados de um subdiretório.

```sh
cppm run
cppm run dev -- "um argumento com espaços"
cppm -C ./outro-projeto run test
```

O `--` separa os argumentos da aplicação das opções do cppm. O status de saída
é preservado para o terminal e para CI. Opções de perfil, offline e lockfile são
herdadas por comandos cppm chamados pelos scripts, assim como `--store-dir`.

## Instalar várias bibliotecas

```sh
cppm install fmt json eigen
cppm add opencv protobuf glm onetbb
```

Cada chamada resolve e instala o conjunto uma vez. Aliases duplicados são
agrupados; versões conflitantes do mesmo pacote são rejeitadas antes da
instalação. Se o Conan falhar, o manifesto e o lockfile anteriores permanecem
intactos. `--cmake-package` e `--target` exigem um único pacote por chamada.

## Armazenamento compartilhado

Instale normalmente com `cppm install fmt`: todos os projetos usam o mesmo
armazenamento por usuário (`~/.conan2` por padrão). Headers e binários
compatíveis são reutilizados diretamente, sem uma cópia em cada projeto.
Pacotes fora do catálogo usam uma versão local quando disponível; para
escolher outra, informe `pacote@versão`.

```sh
cppm store path
cppm store status
cppm store projects
cppm store dedupe
cppm store prune --dry-run
cppm store prune
cppm store prune --sources
cppm --store-dir /outro-disco/bibliotecas install fmt
```

`--store-dir` e `CPPM_STORE_DIR` escolhem a pasta compartilhada e têm precedência
sobre `CONAN_HOME`. Use a mesma pasta nos projetos que devem compartilhar
bibliotecas. A seleção é herdada por scripts; não muda o ambiente global.
O cache existente do Conan continua sendo usado por padrão.

O cppm limpa automaticamente sobras de build e download dos pacotes novos e
compartilha arquivos idênticos por hard links, inclusive entre versões. Arquivos
compartilhados são somente leitura e preservam permissões de execução. Se o
sistema de arquivos não permitir hard links, o arquivo original é mantido.

Cada instalação registra as dependências diretas, transitivas e de build de cada
projeto, configuração e perfil. `store prune` remove pacotes gerenciados sem
referências e sobras antigas. `--dry-run` permite conferir os candidatos sem
remover nada. `--sources` inclui fontes baixadas e pode exigir rede para compilar
uma configuração que ainda não esteja pronta. Aplicações iniciadas com
`cppm exec` prendem as revisões em uso até o fim de sua execução.

Projetos excluídos deixam de prender bibliotecas. Depois de mover um projeto,
execute `cppm install` no novo local antes da próxima limpeza. Projetos existentes
entram no registro na próxima instalação/configuração; pacotes desconhecidos de
outros usuários do Conan são preservados. O cache não é eliminado por idade.

Esses comandos funcionam fora de projetos. `store status --json` inclui projetos,
pacotes e bytes economizados por compartilhamento. O tamanho lógico da home conta
também os nomes ligados por hard links e não mede os blocos físicos; um
armazenamento externo do Conan não entra nesse total da home.

A compatibilidade depende de versão, compilador, opções e configuração.
Debug e Release podem ter binários separados; os builds e executáveis de cada
projeto continuam ocupando espaço próprio. Instalações e limpezas cppm no mesmo
armazenamento aguardam a liberação de um lock compartilhado.

## Builds e testes

```sh
cppm run dev
cppm configure --offline --frozen-lockfile
cppm build --release -j 4
cppm run test
cppm test -- -R app_smoke
```

Debug e Release têm diretórios distintos em `.cppm/build/`. O Conan reutiliza
seu cache compartilhado e o Ninja reutiliza objetos. Builds também reaproveitam
a preparação das dependências quando o perfil resolvido, lockfile, ambiente,
arquivos gerados e pacotes locais continuam válidos. Mudanças de configuração
invalidam o cache automaticamente. O CMake não repete a detecção do compilador
em builds sem alterações. Para forçar uma preparação completa:

```sh
cppm build --fresh
cppm configure --fresh
cppm build --verbose   # informa reaproveitamento e tempos por etapa
```

`cppm install` sempre executa a preparação Conan, mesmo que exista um snapshot
válido. Não é necessário limpar o armazenamento compartilhado para reconfigurar
o projeto.

Instalações e builds publicam `compile_commands.json` na raiz para o clangd
reconhecer os headers instalados. `cppm configure` atualiza essa integração sem
compilar a aplicação. Se já houver um arquivo personalizado ou link na raiz,
ele é preservado; o banco atual fica em `.cppm/compile_commands.json`.
No VS Code com clangd, execute `clangd: Restart language server` se necessário.
Na extensão C/C++ da Microsoft, configure `C_Cpp.default.compileCommands` com
o caminho do banco gerado. A publicação funciona mesmo abrindo a pasta pai no editor.

Novos projetos também incluem tarefas do VS Code. **Ctrl+Shift+B** executa
`cppm build`; **Tasks: Run Task → cppm: run** compila e executa a aplicação com
suas dependências. Em projetos existentes, use `cppm vscode`. Se o VS Code
abriu a pasta pai do projeto, execute `cppm vscode --workspace ..` dentro dele.
O comando preserva arquivos de tarefas personalizados.

Extensões que chamam `g++ arquivo.cpp` diretamente não passam pela configuração
CMake/Conan. Para usar bibliotecas instaladas pelo cppm, selecione as tarefas
cppm ou rode `cppm exec` no terminal. O banco do editor não acrescenta
automaticamente opções ao comando de compilação dessas extensões.

`Target declared` e mensagens OpenCV `[INFO]` são informativas. O cppm usa saída
reduzida por padrão, mantendo avisos e erros. Use `cppm exec --verbose` para
mostrar os detalhes; scripts também herdam essa opção. Uma definição explícita
de `OPENCV_LOG_LEVEL` no ambiente ou no perfil Conan continua sendo respeitada.

`cppm run test` executa o CTest; instalar Catch2 ou Google Test não cria casos
de teste automaticamente. Registre seus testes no CMake.

`exec` e `test` carregam o ambiente de execução das bibliotecas Conan da
configuração escolhida, incluindo DLLs no Windows e plugins Qt. O ambiente
é aplicado apenas aos subprocessos. Abrir o executável por conta própria exige
preparar esse ambiente; distribuir aplicações exige empacotar suas bibliotecas
e recursos separadamente.

## Recompilar ao salvar

```sh
cppm watch
cppm watch test
cppm --offline watch dev -- --port 9090
cppm watch --path src --path include
cppm watch --interval 500 --debounce 400
```

`watch` executa um script do manifesto (`dev` por padrão) e reinicia quando
fontes C/C++, headers, CMake, manifesto ou assets web mudam. Argumentos após
`--` são repassados ao script. A aplicação continua usando o ambiente de suas
bibliotecas, perfil e opções globais. Em WASM, o servidor reinicia na mesma
porta depois de recompilar; atualize a página para ver o resultado.

Alterações próximas são agrupadas para evitar várias compilações por salvamento.
Por padrão, a varredura ocorre a cada 250 ms e aguarda 300 ms sem mudanças antes
de reiniciar. `--path` pode ser repetido para limitar a observação a arquivos ou
pastas específicos; caminhos explícitos também permitem observar outros tipos
de arquivo. `.cppm`, pastas ocultas, `build`, `target`, `node_modules`, `dist`,
`out`, `bin` e o banco de compilação ficam fora da observação padrão. O lockfile
gerado não provoca reinícios durante o próprio build.

Erros de compilação ou de execução mantêm o observador ativo. Corrija e salve
para tentar novamente. Ao reiniciar ou receber Ctrl+C, o observador encerra a
árvore de processos anterior, incluindo servidores iniciados pelo script.
O VS Code oferece a tarefa **cppm: watch**. Na interface interativa, use
**Acompanhar desenvolvimento** ou **Acompanhar testes**.

## Limpar builds

```sh
cppm clean --dry-run
cppm clean debug
cppm clean release
cppm clean obf
cppm clean
```

Sem modo, `clean` remove todas as configurações em `.cppm/build/`. Os modos
`debug` e `release` selecionam os diretórios do target atual, incluindo WASM.
A prévia informa arquivos e bytes lógicos sem removê-los. Fontes, manifesto,
lockfile, dependências Conan e ferramentas em `.cppm/tools/` são preservados.
O banco de compilação gerado é removido apenas se apontar para os builds
excluídos; arquivos personalizados e links do editor são preservados.
A limpeza respeita o lock do projeto e não atravessa links para outros diretórios.

## Build protegido e plugins LLVM

```sh
cppm build obf -j 4
cppm exec obf -- "um argumento"
cppm test obf -- -R app_smoke
```

`obf` gera um Release nativo em `.cppm/build/obf/bin/<nome-do-projeto>` (`.exe`
no Windows). O modo padrão inclui o ofuscador LLVM e aplica mascaramento de
strings locais elegíveis, achatamento de fluxo de controle, substituição de
instruções e divisão de blocos. Também ativa IPO/LTO, reduz símbolos, elimina
informações de depuração e remapeia caminhos do projeto em macros como
`__FILE__`. Os diretórios de Debug/Release normais são independentes.

### Plugin incluído e SDK LLVM

O cppm embute os fontes do [eshard/obfuscator-llvm](https://github.com/eshard/obfuscator-llvm),
revisão `ad26aa21b90cc5c5118d32114b674d1455f54168`, com adaptações documentadas
em `vendor/llvm-obfuscator/UPSTREAM.md`. A licença do componente acompanha o
cppm em `THIRD_PARTY_NOTICES.txt`.

No primeiro build, o cppm compila o plugin contra o SDK instalado e guarda
fontes, objetos e plugin em `.cppm/tools/obfuscator/`. O cache depende da
versão do SDK, do compilador e dos fontes incluídos; sua integridade é
conferida antes do reuso. Um plugin corrompido é reconstruído. Nenhum código
é baixado nesse preparo, inclusive com `--offline`.

São necessários Clang, os headers/bibliotecas de desenvolvimento LLVM e
`llvm-config` da mesma versão. O modo aceita SDKs LLVM 18–23 e foi validado
em Linux com 23.1.4. O host Windows/MSVC não suporta o plugin incluído; macOS
exige um SDK LLVM compatível e ainda não recebeu validação nativa neste
workspace. SDK ausente, compilador incompatível e falha do plugin causam erro;
não há troca silenciosa para o modo básico.

```sh
llvm-config --version
clang++ --version
# Escolha um SDK quando houver várias versões
CPPM_LLVM_CONFIG=/caminho/llvm-config cppm build obf
```

Sem `CC`/`CXX` explícitos, o cppm usa o Clang do SDK escolhido. Se essas
variáveis estiverem definidas, elas precisam apontar a executáveis Clang
compatíveis com o `llvm-config`. Os fontes e `CMakeLists.txt` não são alterados.
O banco de compilação do editor acompanha o último modo configurado.

### Dependências e perfis

Dependências e runtime usam **Release**. Com o perfil `default`, o cppm cria
um perfil Clang local que inclui o perfil original e ajusta compilador,
versão, biblioteca padrão e executáveis do SDK. O arquivo `default` existente
é preservado. Um perfil explicitamente selecionado por `--profile` precisa
usar o Clang correspondente e é validado antes da instalação. Um cache que
só tem binários GCC pode precisar preparar também os binários Clang.

`--store-dir`, `--offline` e `--frozen-lockfile` continuam disponíveis. Os
passes atuam no target executável cujo nome corresponde a `package.name`;
outros targets e bibliotecas Conan não são ofuscados automaticamente. O modo
não suporta WebAssembly. Na interface, use **Comandos → Compilar · Protegido (obf)**.

### Modo básico sem SDK

```sh
cppm build obf --obf-basic
cppm test obf --obf-basic
```

`--obf-basic` dispensa o plugin e mantém o compilador normal: GCC, Clang ou
MSVC. Ativa IPO/LTO, redução de símbolos/debug e remapeamento de caminhos.
No GCC/Clang, usa visibilidade oculta e descarta seções sem uso na linkagem.
No MSVC, usa `/experimental:deterministic`, `/pathmap`, `/DEBUG:NONE`,
`/OPT:REF` e `/OPT:ICF`. Esse modo não transforma o fluxo de controle nem
mascara strings. Também está disponível na paleta como **Reduzir metadados**.
CMake exige suporte funcional a IPO/LTO em vez de ignorar a opção.

### Plugin externo

```sh
CXX=clang++ cppm build obf --obf-plugin /caminho/plugin.so
CXX=clang++ cppm test obf --obf-plugin /caminho/plugin.so
```

Um plugin externo substitui o incluído e continua exigindo o frontend GNU do
Clang. Se houver dependências, use um perfil Conan compatível, por exemplo
`--profile perfil-clang`. A opção é repassada como `-fpass-plugin=<caminho>`;
o plugin deve registrar passes em pontos de extensão automáticos. Plugins
que só registram nomes para `opt -passes=...` precisam implementar esse
registro para Clang. As transformações e a configuração dependem do plugin.
Seu hash invalida os objetos quando o arquivo muda no mesmo caminho.
`--obf-plugin` e `--obf-basic` são mutuamente exclusivos.

### Distribuição, limites e validação

Distribua o executável final, suas bibliotecas e seus recursos. Fontes,
objetos, caches, `compile_commands.json` e símbolos/PDBs são artefatos de
desenvolvimento e ficam fora da distribuição da sua aplicação. Strings ODR
compartilhadas, TLS, literais não elegíveis, metadados externos e caminhos
necessários à execução podem permanecer legíveis. Funções com tratamento de
exceções podem ser puladas por passes que não suportam essa construção.
Mascaramento de strings é ofuscação, não confidencialidade criptográfica.
Nenhuma dessas técnicas torna um binário impossível de analisar ou protege
segredos que a aplicação precisa recuperar. APIs baseadas em símbolos
exportados ou `dlsym` precisam manter exportações explícitas e ser testadas.

Use `cppm test obf` e valide seu fluxo real. A suíte inclui comparação de
comportamento com Release em várias entradas, múltiplas unidades C++,
inicialização estática, threads, exceções, inspeção de literais e do fluxo
LLVM, reuso offline e reconstrução de cache corrompido.

```sh
cargo test --locked --test protection bundled_plugin -- --ignored --nocapture
cargo test --locked --test protection llvm_plugin -- --ignored --nocapture
# Conan e biblioteca compartilhada, em um cache de testes
CONAN_HOME=/tmp/cppm-obf-qa CPPM_TEST_BUNDLED_OBF=1 cargo test --locked --test runtime -- --ignored
```

## Templates

```sh
cppm init app-nativo
cppm init minha-api --template drogon
cppm init meu-wasm --template wasm
```

O template nativo cria uma aplicação de console. Drogon cria uma API com rotas
`/` e `/health`, resposta JSON e teste HTTP real. `run dev` usa a porta 8080;
troque com `cppm run dev -- --port 9090`.

WASM exige um SDK Emscripten instalado e configurado. `run dev` gera HTML, JS e WASM e serve o
resultado em localhost. `run test` usa Node.js. Bibliotecas Conan nativas não
podem ser instaladas nesse target. Python 3 é usado pelo servidor WASM e pelo
teste HTTP do template Drogon.

Antes de compilar WASM, instale o
[SDK Emscripten](https://emscripten.org/docs/getting_started/downloads.html)
e configure-o uma vez com `emsdk activate latest`. Depois, execute no projeto:

```sh
cppm doctor
cppm run dev
```

O cppm detecta SDKs selecionados por `CPPM_EMSDK`/`EMSDK`, ferramentas no `PATH`,
instalações em `~/emsdk`, `~/.local/share/cppm/emsdk` e o SDK do checkout
em `.qa/emsdk`. Para outra instalação, defina `CPPM_EMSDK` com seu diretório.
Não é necessário carregar um script de ambiente a cada novo terminal;
ambientes ativados manualmente também são respeitados. Dentro do projeto
WASM, `doctor` verifica Emscripten, Node.js e Python 3, compila C++ para
WebAssembly e executa o resultado com Node.js. Ele não exige Conan nem um
compilador C++ nativo para esse target.

## Lockfile, offline e perfis

Versione manifesto, lockfile, CMake e fontes. Ignore `.cppm/`.

```sh
cppm install --frozen-lockfile
cppm exec --offline --frozen-lockfile
cppm build --release --profile ./profiles/ci --frozen-lockfile
```

`--frozen-lockfile` exige um lockfile existente e compatível e não o regrava.
Ele fixa receitas e revisões, não o compilador. `--offline` impede consultas
aos remotos Conan, mas uma receita que precise compilar ainda pode baixar
fontes: a opção não bloqueia a rede de subprocessos.

O perfil padrão é detectado somente se estiver ausente. Perfis existentes não
são sobrescritos. Use um perfil explícito para equipes, opções de bibliotecas
e CI, sempre nos passos de instalação e build.

## Problemas comuns

| Sintoma | O que conferir |
| --- | --- |
| `cppm` não encontrado | `~/.local/bin` no PATH no Linux; abra um terminal novo após o instalador Windows. |
| Conan não encontrado | Instale com `uv tool install 'conan>=2.34,<3'` ou pipx e confira o PATH. |
| Não há `cppm.toml` | Entre na pasta do projeto ou use `-C caminho`. |
| Manifesto e lockfile divergem | Execute `cppm install` sem `--frozen-lockfile` e versione o lockfile atualizado. |
| Dependência não existe no cache offline | Faça primeiro uma instalação online no mesmo perfil e configuração. |
| `opencv2/opencv.hpp` ausente ao usar o botão de compilar | Use `cppm build`/`cppm exec` ou as tarefas cppm no VS Code para incluir e ligar as bibliotecas. |
| Conan recusa duas versões | Escolha versões/opções compatíveis para a dependência comum. |
| Receita gráfica pede pacote do sistema | Instale o SDK/pacote de desenvolvimento da plataforma informado pelo Conan. |
| Executável não encontrado | O target CMake executável precisa ter o mesmo nome de `package.name`. |
| Conexão com banco recusada | Confira o servidor, host, porta e autenticação; a instalação fornece o driver cliente. |
| ImGui não abre janela | Configure um backend de janela e renderização e o loop de eventos. |

No Linux, o cppm completa a linkagem de GSSAPI para o libpq 17.11 estático quando
a receita detecta essa biblioteca no sistema. O ajuste coexiste com a correção
de libdecor usada por SDL 3.4 com Wayland.

## Escopo atual

Uma aplicação principal por projeto, dependências com versão exata e backend
Conan 2. Watch, workspaces, registro próprio, cross-compilation e dependências
Conan no WASM ainda não fazem parte desta versão. `add` e `remove` reserializam
o manifesto, portanto comentários do TOML não são preservados.
