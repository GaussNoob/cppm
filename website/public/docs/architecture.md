# Arquitetura

## Interface de terminal

`cppm ui` usa Ratatui/Crossterm e mantém estado de navegação separado da
renderização em `ui`, `ui/view` e `ui/catalog`. A descoberta executa
`conan search '*' --format=json` nos remotos habilitados ou `conan list '*'`
no modo offline. O processo é consultado sem bloquear a navegação; stdout e
stderr vão para arquivos temporários para evitar bloqueio por pipes cheios.
Timeout, saída da interface e execução de uma ação encerram e recolhem esse
processo. A consulta usa modo não interativo para não pedir credenciais por trás
da interface. Resultados parciais preservam as receitas disponíveis e apresentam
as falhas dos remotos.

Instalações, remoções, scripts e outras ações passam por uma prévia e chamam
o próprio executável cppm com argumentos separados e as opções globais atuais.
Assim, a interface reutiliza validação, locks e regras de gravação da CLI.
O terminal sai do modo raw e da tela alternativa durante os comandos e volta
à interface após Enter. Uma guarda restaura o terminal em retornos com erro;
o hook de panic do Ratatui também restaura o estado antes de imprimir o erro.
Os testes cobrem parser de catálogo, seleção, busca, prévias, contexto dos
subprocessos e renderização em diferentes tamanhos.

`python3 scripts/test-ui.py` exercita a interface em um pseudo-terminal Unix:
descoberta com Conan controlado, seleção em lote, cancelamento, rollback de uma
instalação com falha, build Release e CTest reais, scripts com erro, retorno à
interface, redimensionamento, cores, restauração do terminal e encerramento de
uma consulta lenta. `--catalog arquivo.json` usa uma resposta real previamente
capturada de `conan search '*' --format=json`, sem novo acesso à rede. O CI
Linux executa esse teste após a suíte Rust.

A CLI coordena ferramentas existentes. O cppm define o formato do projeto,
as convenções de uso e a integração entre elas; Conan resolve pacotes e
compatibilidade de binários, CMake configura targets e Ninja compila.

## Responsabilidades

| Módulo | Responsabilidade |
| --- | --- |
| `cli` | Argumentos, despacho de comandos e saída para o usuário |
| `manifest` | Modelo do projeto, leitura TOML e validação |
| `packages` | Catálogo pequeno e mapeamento de pacotes para targets CMake |
| `project` | Descoberta da raiz, criação do projeto e exclusão mútua |
| `dependencies` | Operações de adicionar/remover/instalar, lockfile e integração CMake |
| `conan` | Adaptação do manifesto à CLI do Conan 2 |
| `platform` | Ambiente nativo de build; descoberta e ativação MSVC no Windows |
| `protection` | Preset obf, injeção CMake, validação de aplicação e plugins LLVM |
| `doctor` | Versões das ferramentas e prova de compilação/execução C++ com relatório JSON |
| `runtime` | Ambiente de execução Conan nos subprocessos de exec e CTest |
| `build` | Configuração CMake, build Ninja, execução e CTest |
| `process` | Execução de subprocessos, scripts, ambiente e status de saída |
| `storage` | Gravação atômica de arquivos individuais |
| `store` | Localização, inspeção, exclusão mútua e limpeza do cache compartilhado Conan |
| `store::registry` | Referências por projeto/configuração, proteção de aplicações ativas e coleta de pacotes sem uso |
| `store::content` | Objetos BLAKE3, compartilhamento por hard links e estatísticas de bytes únicos |
| `editor` | Publicação do banco de compilação na raiz, preservando arquivos personalizados |
| `templates` | Arquivos iniciais e convenções de app, API Drogon e WASM |

Os modelos são estruturas concretas. Ainda existe um único backend de pacotes
e um único caminho de build, portanto não há traits ou factories para backends
hipotéticos. A separação do adaptador Conan permite introduzir outra
implementação quando houver um segundo caso real.

## Adicionar uma dependência

1. Adquire o lock do projeto e relê o manifesto.
2. Normaliza nome e aliases; resolve a versão com Conan quando não há uma explícita ou padrão.
3. Monta o novo manifesto em memória.
4. Instala o grafo com Conan e captura lockfile e metadados em uma pasta temporária.
5. Usa os metadados `cmake_file_name` e `cmake_target_name` para gerar a integração.
6. Grava o manifesto e os arquivos de integração e lockfile.

Falhas de resolução ou compilação da dependência não alteram o manifesto nem o
lockfile versionado. Conan pode ter adicionado arquivos ao seu cache ou à pasta
interna de geradores antes de falhar. O próximo comando reinstala a integração.

Cada arquivo é escrito em um temporário no mesmo filesystem e renomeado.
Isso evita arquivos truncados; não representa uma transação entre vários
arquivos. Um erro de disco durante a etapa de gravação ainda pode exigir
`cppm install` para reconciliar manifesto e lockfile.

## Concorrência e processos

Um lock de arquivo do sistema operacional protege operações que alteram o
projeto. A leitura sob o lock evita usar um manifesto anterior à última
operação concluída. O sistema libera o lock quando o processo termina, mesmo
sem encerramento normal. Comandos concorrentes recebem um erro e podem ser
repetidos.

Um segundo lock em `<CONAN_HOME>/cppm-store.lock` serializa a detecção do
perfil, resolução, instalação e limpeza de dependências entre projetos. A
espera só ocupa a etapa Conan; os builds de aplicações podem ocorrer em
paralelo depois da instalação. O lock é liberado pelo sistema operacional ao
encerrar o processo. Ele coordena comandos cppm; chamadas diretas ao Conan
precisam ser coordenadas por quem as executa.

`run` não segura o lock enquanto executa um script, pois scripts podem chamar
outros comandos cppm. O ambiente registra a cadeia de scripts para detectar
recursão. `exec` libera o lock antes de iniciar a aplicação.

Comandos internos usam argumentos de subprocesso, sem concatenação em shell.
Scripts do manifesto em array usam execução direta, inclusive no Windows.
Scripts em string usam `sh` no Linux ou Windows PowerShell no Windows. Argumentos
externos chegam por parâmetros posicionais no Unix e por JSON e splatting no
PowerShell; não são interpolados no código do script. Para argumentos nativos
com aspas ou regras especiais, os arrays evitam as limitações do shell.
O ambiente de cada subprocesso é configurado diretamente no comando,
sem mutações globais de ambiente no processo Rust.

## Build protegido

`build obf`, `exec obf` e `test obf` usam um plugin incluído por padrão.
`protection/bundled` embute fontes e licença de uma revisão fixa do
Obfuscator-LLVM, detecta o SDK LLVM selecionado por `CPPM_LLVM_CONFIG` e
compila um módulo contra seus headers, ABI e Clang. O cache fica no projeto,
em `.cppm/tools/obfuscator/`, separado por fontes e identidade do SDK. Um selo
BLAKE3 confere o binário; corrupção provoca relinkagem. Não há download no
bootstrap. As variáveis de pipeline são aplicadas apenas ao subprocesso que
compila a aplicação, sem alterar o ambiente global nem os builds Conan.

O perfil Conan automático inclui o default e substitui apenas a seleção do
compilador/libcxx e seus executáveis. Perfis explícitos são validados. O build
retorna as opções efetivas para a execução e para o lease do armazenamento,
conservando exatamente as revisões da configuração Clang em uso. O perfil
original não é sobrescrito. Dependências e runtime usam Release; os objetos
protegidos têm uma pasta própria.

O hook `CMAKE_PROJECT_INCLUDE` adia as opções até o target executável existir,
ignora probes/subprojetos e valida IPO/LTO. Um marcador reiniciado confirma a
aplicação da injeção. `--obf-basic` preserva o compilador normal e desativa o
plugin; `--obf-plugin` usa um plugin externo sem impor o pipeline incluído.
O digest do plugin invalida objetos após uma troca no mesmo caminho.

As adaptações do ofuscador limitam strings a arrays locais de bytes não TLS,
evitando decodificação dupla de constantes ODR compartilhadas. Helpers são
internos e o decoder mantém uma fronteira noinline/optnone durante LTO. Os
testes verificam semântica C++, ausência de um literal de controle, mudanças
no IR, reuso/corrupção do cache e integração Conan/runtime com lockfile congelado.

## Windows

O módulo `platform` respeita um Developer Prompt já ativo ou um compilador
escolhido em `CC`/`CXX`. Caso contrário, encontra Visual Studio com `vswhere`,
executa `VsDevCmd.bat` e captura o ambiente em UTF-16. Esse ambiente é aplicado
apenas aos subprocessos de build e guardado em memória durante o comando.
O caminho descoberto é passado por variável de ambiente para um comando cmd
fixo. A aplicação usa a extensão de executável da plataforma.

Os scripts de instalação são independentes da CLI. O instalador Windows copia
o executável para uma pasta do usuário e atualiza seu PATH persistente. O
empacotamento inclui instalador, binário, licença, instruções e SHA-256, sem
publicação automática. O CI inclui builds e testes nativos em Linux e Windows.

## Instalação em lote e ambiente de execução

`add` e `install` normalizam e validam todos os pacotes recebidos antes de
consultar o Conan. Aliases duplicados são agrupados, versões conflitantes são
rejeitadas e uma única preparação de dependências antecede a gravação do
manifesto/lockfile. Uma falha de resolução ou instalação preserva ambos.

A receita Python consumidora gera CMakeDeps, CMakeToolchain e VirtualRunEnv.
O ambiente VirtualRunEnv também é exportado para `cppm-runtime.json`, com
marcadores de herança em vez de capturar os valores do ambiente do usuário.
`exec` e `test` aplicam esse JSON ao subprocesso, sem executar comandos de shell.
O PATH preparado pelo MSVC é preservado ao acrescentar os caminhos das DLLs.
Receitas podem definir variáveis como `QT_PLUGIN_PATH`; perfil e configuração
continuam sob controle do Conan. Debug e Release usam arquivos separados.

A CLI captura o ambiente antes de soltar o lock do projeto em `exec`. Remover
a última dependência também deixa de aplicar qualquer ambiente antigo. O cppm
não empacota as aplicações do usuário nem substitui ferramentas de implantação
de frameworks.

## Verificação

Os testes locais chamam o binário da CLI em diretórios temporários e usam
CMake, Ninja e um compilador reais. Eles cobrem criação sem sobrescrita,
descoberta da raiz, execução, argumentos, status de saída, Debug/Release,
CTest, lockfile congelado, concorrência e falha de instalação. Um Conan
controlado com falha permite verificar que `add` preserva os arquivos originais.

O teste de integração separado utiliza Conan e fmt reais, com compilação e
reutilização offline dos artefatos. Não depende de mock para validar a
integração de pacotes.

O teste `store` exporta uma receita C++ local em um cache isolado e desativa
todos os remotos. Dois projetos resolvem a mesma receita, compilam e executam
aplicações usando o mesmo diretório de pacote. Um registro feito pela receita
comprova uma única compilação da biblioteca e a remoção automática de seu build.
Ambos os projetos passam no CTest offline após as duas modalidades de limpeza.
O teste verifica também hard links entre versões e configurações, integridade
Conan, prévia sem alterações e coleta depois de atualização, remoção e exclusão
de projetos. Testes locais cobrem leases de execução, metadados inválidos,
permissões de execução, symlinks e rejeição de objetos corrompidos.

## Builds incrementais

Builds comuns consultam um snapshot local em
`.cppm/conan/<modo>/cppm-install.json` antes de repetir `conan install`.
A chave inclui a receita do consumidor, versão/home Conan, perfis host/build
resolvidos por `conan profile show --format=json`, ambiente, configuração global
e registro de receitas editáveis.
Includes e expressões dos perfis são resolvidos pelo próprio Conan. A chave
persiste apenas hashes dos perfis e do ambiente consultado.

O snapshot guarda os metadados do grafo necessários à integração/registro e o
lockfile; ambientes e configurações extras do relatório Conan são descartados.
Ele só é reutilizado se o lock atual coincidir,
os arquivos dos geradores continuarem idênticos e os manifests dos pacotes locais
estiverem disponíveis e inalterados. Receitas editáveis não usam esse atalho.
Validação, instalação e registro continuam dentro do lock do armazenamento;
o reaproveitamento também registra o grafo para coleta e leases de execução.
Metadados ausentes ou inválidos provocam uma instalação normal. `install`, `add`
e `remove` sempre preparam dependências; builds com `--fresh` também.

Uma segunda assinatura, em `cppm-configure.key`, descreve os argumentos e ambiente
CMake, integração de dependências, arquivos gerados, CMake principal, shell HTML
e identidade/metadados do compilador. Quando ela muda, a configuração usa
`--fresh`. Sem mudanças, o build delega ao Ninja a verificação de fontes, headers,
globs e arquivos CMake incluídos. A publicação do banco de compilação ocorre
depois do build, inclusive quando a compilação falha após uma regeneração.
`configure` sempre executa CMake; `configure --fresh` força a recriação do cache.
O preset protegido sempre reaplica e verifica seu hook, mesmo com assinatura igual.

Registros, integração e snapshots idênticos usam gravação condicional para evitar
renames e sincronizações de disco desnecessários. `--verbose` informa os tempos
de preparação, configuração e compilação.

## Observação e limpeza de builds

`watch` mantém um snapshot de tamanho e data de modificação dos arquivos relevantes
e agrupa mudanças por um período de debounce. Executa a CLI em um subprocesso,
preservando opções globais e a execução normal dos scripts. No Unix, cada execução
fica em um grupo próprio: o encerramento envia SIGTERM e, se necessário, SIGKILL.
No Windows, o encerramento usa a árvore de processos via `taskkill /T`.
O handler de Ctrl+C permite encerrar filhos antes de sair com status 130;
SIGTERM/SIGHUP também são tratados no Unix. Diretórios gerados e links para
diretórios não são percorridos. Falhas do script aguardam uma nova alteração.

`clean` opera apenas em caminhos fixos sob `.cppm/build/`, com o lock do projeto.
A contagem usa metadados sem seguir symlinks. A prévia não exclui arquivos.
Na exclusão, o editor remove apenas bancos reconhecidos como gerados e referentes
à configuração removida. O armazenamento Conan e os SDKs locais são preservados.

## Armazenamento de dependências

`--store-dir`/`CPPM_STORE_DIR` configuram `CONAN_HOME` apenas nos subprocessos.
A CLI transforma caminhos relativos em absolutos antes de descobrir a raiz
do projeto e preserva a escolha nos scripts. Sem essas opções, usa `CONAN_HOME`
ou a localização padrão retornada por `conan config home`, aproveitando o cache
existente. O cppm não cria uma segunda cópia dos pacotes nem uma árvore local
de links. CMakeDeps e VirtualRunEnv apontam para os artefatos do cache.

A resolução de versões não força `--update`: Conan prefere receitas locais e
consulta remotos quando necessário. A compatibilidade dos binários continua
sob controle do Conan, incluindo compilador, configurações e opções.

Depois de instalar, o grafo seleciona com `conan list --graph` somente os
binários recém-compilados/baixados. `conan cache clean --list` remove seus
builds e downloads. Uma falha nessa limpeza gera um aviso e mantém a instalação
válida. Não há limpeza automática de pacotes antigos ou de fontes.

`store::Pending` mantém o lock global desde a instalação até o commit do projeto.
Somente depois da gravação do manifesto/lockfile, as referências exatas do grafo
são registradas atomicamente em `<home>/cppm-store/index.json`. O registro inclui
revisões de receitas, binaries e python_requires, em ambos os contextos. Cada
projeto guarda snapshots por perfil e modo; uma mudança de dependências invalida
os snapshots anteriores. Remover a última dependência libera as referências em
todos os stores registrados no projeto, sem precisar executar Conan.

Após registrar, novos arquivos de payload entram em um pool endereçado por
conteúdo BLAKE3 e permissões de leitura/execução. Hard links substituem somente
arquivos com conteúdo verificado, via rename atômico no diretório de destino.
Os bytes e metadados de identificação Conan permanecem iguais. O payload passa
a ser somente leitura, conforme o contrato do cache de pacotes; links simbólicos
não são seguidos nem alterados. Falhas de suporte a hard links deixam os arquivos
originais intactos. `store dedupe` repete a operação para pacotes já registrados.

`exec` cria um lease com as revisões exatas antes de liberar o lock do projeto.
O lock do lease é separado de seus metadados para permitir leitura no Windows.
Um lease vivo mantém as dependências até a aplicação terminar; leases abandonados
não prendem pacotes e são removidos durante a coleta.

`store prune` valida os registros, reúne referências de projetos existentes e
leases ativos e consulta todas as revisões do Conan. Somente artefatos gerenciados
sem referências são removidos, por `conan remove`, nunca por exclusão manual de
diretórios do backend. Receitas com binários desconhecidos são preservadas.
Objetos sem links restantes são coletados depois. `--dry-run` não altera o índice,
pacotes nem arquivos compartilhados. A limpeza Conan de builds, downloads e
temporários continua disponível; `--sources` inclui fontes.

`store status` mede bytes lógicos e únicos dos payloads gerenciados. No Unix,
usa identidade de inode e contador de links, sem reler bibliotecas grandes.
O tamanho lógico da home inclui nomes de hard links e não representa blocos
físicos. Armazenamento externo do Conan não entra nesse total da home.

## Integração com editores

`cppm configure` prepara dependências e gera CMake sem compilar a aplicação.
Instalações chamam essa configuração como etapa posterior, com aviso recuperável
se o CMake falhar. Builds também publicam o banco de compilação na raiz, onde o
clangd o encontra ao procurar nos ancestrais do fonte. Uma cópia anterior em
`.cppm/compile_commands.json` distingue o arquivo gerado de alterações do usuário.
Arquivos personalizados e symlinks na raiz são preservados. O arquivo gerado é
acrescentado ao `.gitignore` do projeto.

## Templates e WebAssembly

O template escolhe arquivos e configuração no momento de `init`. O manifesto
registra `package.target = "wasm"` quando necessário. Builds WASM usam `emcmake`
para configurar a toolchain Emscripten, mantendo CMake e Ninja como backend.
Debug e Release ficam em diretórios separados dos builds nativos.

A seleção do SDK respeita `CPPM_EMSDK`, `EMSDK` e ferramentas no `PATH`, nessa
ordem. Também procura instalações convencionais no diretório do usuário e o SDK
associado ao executável/checkout local (`.qa/emsdk`). A configuração `.emscripten`
é lida com Python, como no próprio Emscripten, preservando expressões e caminhos.
O ambiente com SDK, Node.js e Python é aplicado apenas aos comandos de WASM,
incluindo a compilação Ninja, CTest e servidor. Nenhuma variável global ou arquivo
de inicialização do shell é alterado. O hash CMake também considera o conteúdo
da configuração do SDK, para invalidar mudanças no mesmo caminho.

Antes de preparar dependências, o build WASM verifica `em++ --version` e
`emcmake cmake --version`. O segundo comando valida o wrapper, já que
`emcmake --version` apenas imprime ajuda e retorna erro. A falha inclui
instruções de instalação e ativação do SDK para Bash/zsh, fish e Windows.
O `doctor` seleciona as verificações pelo target: projetos WASM verificam
Emscripten, Node.js e Python 3 e compilam e executam um módulo com Node.js,
sem exigir Conan ou inicializar a toolchain C++ nativa. `EMSDK_NODE` é
respeitado quando definido.

O template WASM produz HTML, JavaScript e o módulo WASM. O servidor de
desenvolvimento usa o módulo HTTP da biblioteca padrão do Python, limitado a
localhost e à pasta de saída do build. A biblioteca padrão C++ é suportada;
dependências Conan são recusadas nesse target até haver perfis e resolução
separados para Emscripten. Isso impede reutilizar binários nativos por engano.

Drogon mantém o caminho nativo de dependências. Seu teste gerado inicia o servidor
em uma porta local temporária, verifica `/health` e encerra o processo no final.

## Compatibilidade de receitas

A integração mantém um ajuste explícito para SDL 3.4 estático com Wayland no
Linux: o build SDL pode descobrir libdecor do sistema sem que a receita Conan
propague esse vínculo. `cmake/sdl-wayland.cmake` acrescenta o target pkg-config
de libdecor quando disponível. Esse ajuste é limitado ao pacote, versão e
configuração afetados, e é verificado pelo teste real SDL.

O libpq 17.11 estático compilado com Meson pode detectar GSSAPI do sistema sem
propagar essa dependência na receita. `cmake/postgresql-gssapi.cmake` procura a
biblioteca via pkg-config ou `find_library` e completa a linkagem no Linux.
Os ajustes de SDL e PostgreSQL são cumulativos. Builds compartilhados e receitas
no contexto build não recebem o ajuste de libpq.

O catálogo tem versões iniciais fixas, aliases e nomes CMake para consulta.
A instalação continua descobrindo a integração pelos metadados da receita,
inclusive para pacotes com múltiplos componentes como Qt, Boost e Protobuf.
Os aliases de banco selecionam bibliotecas cliente. A CLI não administra
servidores nem provisiona bancos.

No Linux, o build Conan do Qt usa o dialeto GNU do padrão C++ declarado. Isso
habilita as extensões de inteiros exigidas pelo Qt, incluindo `__int128`.
A configuração é limitada ao pacote Qt; o CMake da aplicação continua usando
o padrão ISO declarado, sem habilitar extensões para o código do usuário.

Para dependências diretas de OpenCV, o conanfile gerado define
`opencv/*:with_ffmpeg=False` como padrão inicial. Perfis do Conan podem
sobrescrever essa opção. Isso mantém a instalação básica voltada a imagens
sem exigir toda a cadeia de codecs de áudio e vídeo.

No Linux nativo, a mesma receita consumidora define `opencv/*:with_v4l=True`.
Isso permite capturar webcams comuns por V4L2 sem instalar FFmpeg ou plugins
GStreamer. A opção é um default, respeita uma escolha explícita do perfil e não
é emitida no Windows, onde a receita não oferece `with_v4l`. O teste OpenCV
verifica a disponibilidade do backend no Linux sem precisar de hardware.

O padrão do cppm para Qt é `shared=True`. Isso mantém os detalhes de bibliotecas
internas do framework dentro de suas bibliotecas dinâmicas, inclusive partes
ausentes nos metadados estáticos da receita Qt 6.12. Um perfil Conan pode
sobrescrever essa opção, assumindo a configuração de linkagem estática.

O plugin PostgreSQL do Qt é desativado no padrão (`with_pq=False`), porque a
receita Qt não completa os vínculos necessários ao libpq estático. O driver
libpq instalado diretamente pelo cppm continua disponível à aplicação.

Para Protobuf nativo, `cmake/protobuf-protoc.cmake` aponta `protobuf::protoc`
para o compilador incluído no mesmo pacote do runtime. Isso evita selecionar
um `protoc` incompatível previamente instalado no PATH. Os builds com schemas
são verificados gerando, compilando e serializando uma mensagem real.

