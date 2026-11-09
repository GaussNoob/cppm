# Instalação do cppm

Baixe a [última release](https://github.com/GaussNoob/cppm/releases/latest).
Os pacotes não exigem Rust no computador que vai usar a ferramenta.

## Instalar direto do GitHub

No Linux x64 ou ARM64, com `curl`, `tar` e `sha256sum` disponíveis:

```sh
curl -fsSL https://raw.githubusercontent.com/GaussNoob/cppm/main/scripts/bootstrap.sh -o install-cppm.sh
sh install-cppm.sh
```

O destino padrão é `~/.local/bin/cppm`. Para escolher outra pasta ou uma versão
específica, use `sh install-cppm.sh --prefix "$HOME/programas" --version 0.4.2`.
Se necessário, adicione `~/.local/bin` ao PATH do shell. No fish, execute
`fish_add_path ~/.local/bin`; no Bash/Zsh, acrescente `export PATH="$HOME/.local/bin:$PATH"`
ao arquivo de configuração do shell e abra outro terminal.

No Windows x64, use Windows PowerShell 5.1 ou superior:

```powershell
Invoke-WebRequest -UseBasicParsing https://raw.githubusercontent.com/GaussNoob/cppm/main/scripts/bootstrap.ps1 -OutFile install-cppm.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\install-cppm.ps1
```

O script instala a CLI em `%LOCALAPPDATA%\Programs\cppm\bin` e configura o
PATH do usuário. Use `-InstallDir 'D:\Minhas ferramentas\cppm'` para personalizar,
`-NoPath` para preservar o PATH, ou `-Version 0.4.2` para fixar uma release.
`-WithDependencies` também prepara Python, Conan, CMake, Ninja, MSVC/Windows SDK
e Emscripten/Node; essa opção pode pedir administrador para o compilador.
Abra um terminal novo e execute `cppm doctor`.

Os scripts consultam o arquivo `VERSION` da última release, escolhem o pacote
da arquitetura e verificam seu SHA-256 e a versão do executável antes de instalar.
Os downloads vêm de HTTPS no GitHub. As releases antigas sem `VERSION` precisam
ser baixadas manualmente; a instalação remota fica disponível após publicar
a primeira release com o workflow descrito abaixo. Os pacotes Linux usam glibc;
Alpine/musl exige compilação local. O workflow publica Linux x64/ARM64 e Windows x64.

## Atualizar o cppm

```sh
cppm update --check  # consulta imediatamente, sem instalar
cppm update          # baixa, verifica e instala a nova versão
```

Esses comandos funcionam fora de projetos. `cppm update` atualiza o executável
no diretório da instalação atual, inclusive instalações personalizadas, e mantém
as ferramentas privadas, o PATH e os projetos. A pasta precisa permitir escrita
ao usuário. Uma versão local mais recente que a release não é substituída.
No Linux, a substituição é atômica. No Windows, o executável anterior é renomeado
para permitir atualizar enquanto está em uso e restaurado se a substituição falhar.
Uma cópia `cppm.exe.previous` ainda em uso é removida na próxima instalação ou
desinstalação. Os próximos comandos usam a nova versão; reabra a interface
interativa se ela estiver aberta durante a atualização.

Ao executar comandos em um terminal, a ferramenta consulta atualizações em
segundo plano, sem esperar a rede, no máximo uma vez a cada seis horas.
Quando já existe uma versão nova no cache, cada uso mostra:

```text
cppm: Atualização disponível: 0.4.2 → 0.4.3. Execute `cppm update`.
```

Na primeira consulta, o aviso aparece na próxima execução; `cppm ui` também
acompanha o resultado e mostra um aviso no cabeçalho. A aba Comandos oferece
**Atualizar cppm**. Falhas de rede ficam silenciosas e são tentadas novamente
após uma hora. A consulta automática não roda com `--offline`, saída redirecionada,
`CI=true` ou `CPPM_NO_UPDATE_CHECK=1`. O comando explícito `update --check`
sempre consulta a rede, exceto com `--offline`, que retorna uma mensagem de erro.
O cache fica em `$XDG_CACHE_HOME/cppm/update.json` (ou `~/.cache/cppm/update.json`)
no Linux e `%LOCALAPPDATA%\cppm\update.json` no Windows.

## Windows x64

O caminho recomendado é abrir `cppm-0.4.2-windows-x86_64-setup.exe` e seguir o
assistente. Ele instala o cppm, baixa Python 3.13, Conan 2, CMake, Ninja,
Emscripten 6.0.13 e seu Node.js em um ambiente próprio e instala MSVC + Windows SDK se faltarem. Configura o PATH do
usuário e valida a compilação C++ antes de concluir. Depois, abra um terminal novo:

```powershell
cppm doctor
cppm init meu-app
cd meu-app
cppm run dev
```

A instalação exige Internet. O compilador e o Windows SDK podem baixar vários
GB e pedir autorização de administrador. Se a Microsoft indicar que é preciso
reiniciar, o assistente solicita a reinicialização antes do primeiro build.
Python e as ferramentas ficam em `%LOCALAPPDATA%\Programs\cppm\tools`, sem
depender de uma instalação anterior de Python ou de winget/Chocolatey. O log
fica em `%LOCALAPPDATA%\Programs\cppm\install.log`. O cppm aparece em
**Configurações → Aplicativos instalados**; a desinstalação remove suas ferramentas
privadas e preserva projetos, cache de pacotes Conan e o compilador compartilhado.

O instalador 0.4.1 preparava apenas C++ nativo. Se um projeto WASM criado com ele
acusar SDK ausente, execute o instalador atual sobre a instalação existente e
abra outro terminal. Dentro do projeto, use `cppm doctor` e `cppm run dev`;
não execute `cppm init` novamente. O SDK privado fica em `tools\emsdk` e é
localizado automaticamente, mesmo com uma pasta de instalação personalizada.

Consulte o [guia Windows](windows.md) para checksums e detalhes. O pacote
`cppm-0.4.2-windows-x86_64.zip` continua disponível para instalar somente a CLI:

Extraia o pacote Windows e execute no PowerShell, dentro da pasta extraída:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

O instalador copia `cppm.exe` para `%LOCALAPPDATA%\Programs\cppm\bin` e registra
esse diretório no `PATH` do usuário, sem privilégios de administrador. Abra um
novo terminal e execute:

```powershell
cppm --version
cppm doctor
```

É possível escolher outra pasta com `-InstallDir` ou omitir a alteração do
PATH com `-NoPath`. Para desinstalar a CLI, execute o `uninstall.ps1` do pacote,
usando o mesmo `-InstallDir` se ele tiver sido personalizado. Projetos, cache
Conan e compiladores são preservados.

Para compilar aplicações, instale Visual Studio Build Tools com a carga
**Desktop development with C++**, incluindo MSVC e Windows SDK. Instale também
CMake 3.24+, Ninja e Conan 2.34+. O cppm encontra o Visual Studio com `vswhere`
e carrega o ambiente MSVC nos subprocessos. Um Developer PowerShell ou um
compilador explícito em `CXX`/`CC` é respeitado.

Se você está usando o repositório em vez de um pacote, instale Rust, compile
e execute o instalador local:

```powershell
cargo build --release --locked
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

O workflow de releases compila o ZIP e o assistente Windows com MSVC.
Pacotes GNU x64 também podem ser gerados localmente, com validação parcial no Wine.
`windows-installer.yml` testa a instalação do assistente
e a compilação C++ nativa pelo PowerShell. Os testes adicionais de integração
com bibliotecas ficam no workflow `ci.yml`.

## Linux

Dentro do pacote extraído:

```sh
sh install.sh
```

O destino padrão é `~/.local/bin/cppm`. Use `--prefix /outra/pasta` para instalar
em `/outra/pasta/bin/cppm`. O instalador não edita arquivos de configuração do
shell. Se a pasta ainda não estiver no PATH, adicione-a permanentemente; no fish:

```fish
fish_add_path ~/.local/bin
```

No Linux, instale CMake 3.24+, Ninja e GCC ou Clang pelo gerenciador da distribuição.
Para Conan, use `uv tool install 'conan>=2.34,<3'` ou pipx. No repositório, o
instalador também encontra `target/release/cppm` depois de `cargo build --release`.

## Gerar pacotes

Na plataforma de destino, com Python 3.11+ e Rust instalados:

```sh
cargo build --release --locked
python scripts/package.py
```

São gerados um `.zip` no Windows ou `.tar.gz` no Linux, acompanhados do SHA-256.
Para empacotar uma compilação cruzada da CLI, use por exemplo
`python scripts/package.py --target x86_64-pc-windows-gnu` depois do build com
o mesmo `--target`. Cada pacote contém um `release.json` com o target utilizado.
O workflow `ci.yml` compila, testa e empacota em runners Linux e Windows;
esses artefatos ficam no próprio workflow. O workflow `release.yml` publica
pacotes e checksums no GitHub Releases quando recebe uma tag de versão.
Ainda não há pacote winget/Scoop.

Para gerar o assistente Windows, instale NSIS 3 e execute:

```sh
python scripts/package-windows-installer.py
```

Em Linux, informe `--binary target/x86_64-pc-windows-gnu/release/cppm.exe`
após o build cruzado. `--makensis` seleciona o compilador NSIS. O workflow
`windows-installer.yml` gera o `.exe` e testa instalação, reinstalação, PATH em
terminal novo, build C++ real e desinstalação em um runner Windows. Os downloads
do uv são fixados por versão e SHA-256; o bootstrapper MSVC só é executado com
assinatura Authenticode válida da Microsoft. Veja as versões de ferramentas
em `scripts/windows-tools.json`.

## Publicar uma atualização

1. Atualize `package.version` no `Cargo.toml` e o registro do cppm no `Cargo.lock`.
   Use uma versão estável `X.Y.Z` maior que a anterior.
2. Envie o commit com o código e os workflows para o GitHub.
3. Crie e envie uma tag que corresponda exatamente à versão, por exemplo:

```sh
git tag v0.4.2
git push origin v0.4.2
```

O workflow `.github/workflows/release.yml` valida a tag contra o `Cargo.toml`,
compila e testa Linux x64, Linux ARM64 e Windows x64, e gera os pacotes e o
assistente Windows. Só após todos os builds passarem, publica a release com
notas automáticas, checksums SHA-256 e o arquivo `VERSION`, marcando-a como
`latest`. Durante os uploads a release permanece em rascunho, evitando que
instaladores enxerguem uma versão incompleta. O token automático do Actions
recebe `contents: write` somente no job de publicação; não é preciso criar um PAT.
Uma release já publicada não é sobrescrita ao repetir o workflow.

Um push comum de código não dispara avisos: é necessário publicar a release
com a tag. As instalações que já incluem o módulo de atualização descobrem
a nova versão na próxima consulta automática. Quem usa uma versão anterior
sem esse módulo precisa instalar uma vez pelo script ou pelo novo pacote.

Para validar o fluxo sem rede:

```sh
cargo test --locked
python scripts/test-update.py --binary target/debug/cppm
```

No Windows, após `cargo build --release --locked`, execute
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-bootstrap.ps1`.
Os testes substituem apenas o transporte por arquivos locais e verificam
instalação, checksums, versão, atualização e preservação da instalação em falhas.

## Pacotes C++

Para projetos sem framework, use `cppm init meu-app`. Também há templates:

```sh
cppm init minha-api --template drogon
cppm init meu-wasm --template wasm
```

No Drogon, `run dev` inicia uma API local com `/health` e `run test` valida uma
resposta HTTP real. O Conan instala Drogon 1.9.14 e suas dependências.

No WASM, o instalador completo Windows prepara o SDK Emscripten automaticamente.
Em outras instalações, instale o SDK e configure-o uma vez com `emsdk activate latest`.
O cppm localiza o SDK automaticamente. `run dev` compila e serve
o HTML em localhost; `run build` gera os artefatos em
`.cppm/build/wasm-release/bin/`; `run test` usa Node.js. A porta do servidor pode
ser alterada com `cppm run dev -- --port 9090`. Dependências Conan nativas não
podem ser ligadas ao módulo WASM nesta versão.

Uma alternativa sem Git, com Python 3.11+ e o repositório disponível, é executar
`python scripts/setup-wasm.py`. O script baixa uma revisão fixa do SDK, verifica
o SHA-256 do arquivo de origem e instala em `~/.local/share/cppm/emsdk`. Depois,
compila e executa um módulo WASM com o Node privado, sem alterar o ambiente global.
Também é possível seguir as instruções do
[SDK Emscripten](https://emscripten.org/docs/getting_started/downloads.html).
Com o SDK instalado, execute no seu projeto:

```sh
cd meu-wasm
cppm doctor
cppm run dev
```

O cppm respeita `CPPM_EMSDK` e `EMSDK`, ferramentas já disponíveis no `PATH`,
SDKs em `~/emsdk`, `~/.local/share/cppm/emsdk` e o SDK do checkout local
em `.qa/emsdk`. Para um SDK em outro diretório, informe `CPPM_EMSDK`.
O ambiente é aplicado apenas aos subprocessos de WASM: você pode abrir outro
terminal e executar `cppm run dev` diretamente. Ativação manual continua válida.
O diagnóstico de projetos WASM verifica Emscripten, Node.js e Python 3 e compila
e executa um módulo WebAssembly. Ele não exige as ferramentas nativas de Conan
ou MSVC/GCC nesse target. Builds também verificam o SDK antes de preparar
dependências ou gravar o lockfile, com instruções de instalação/configuração
quando nenhum SDK válido for encontrado.

```sh
cppm init meu-app
cd meu-app
cppm install sqlite
cppm install json
cppm install curl
cppm install sdl
cppm run dev
```

`sqlite` é um alias de `sqlite3`, `json` de `nlohmann_json`, e `curl` de `libcurl`.
JSON usa o header `<nlohmann/json.hpp>` e o tipo `nlohmann::json`.
`sdl` consulta a versão
estável disponível no Conan; `sdl2` e `sdl3` selecionam padrões explícitos de cada
geração. O manifesto recebe uma versão exata e o lockfile fixa o grafo resolvido.
Os metadados CMake da receita configuram includes e linkagem.

O [catálogo organizado](packages.md) inclui MySQL/MariaDB, PostgreSQL, Redis,
MongoDB, SQLite e bibliotecas de rede, interfaces, testes, imagens, matemática,
serialização e paralelismo. O [guia de uso](guide.md) cobre scripts, templates,
perfis, lockfile e problemas comuns.

Bibliotecas com integração ao sistema, como SDL, podem exigir SDKs e bibliotecas
de desenvolvimento da plataforma. Isso é informado pela receita Conan durante
a instalação; o cppm não instala pacotes do sistema com privilégios elevados.
