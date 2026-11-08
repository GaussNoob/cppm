# Instalação do cppm

Baixe a [última release](https://github.com/GaussNoob/cppm/releases/latest).
Os pacotes não exigem Rust no computador que vai usar a ferramenta.

## Windows x64

O caminho recomendado é abrir `cppm-0.4.1-windows-x86_64-setup.exe` e seguir o
assistente. Ele instala o cppm, baixa Python 3.13, Conan 2, CMake e Ninja em um
ambiente próprio e instala MSVC + Windows SDK se faltarem. Configura o PATH do
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

Consulte o [guia Windows](windows.md) para checksums e detalhes. O pacote
`cppm-0.4.1-windows-x86_64.zip` continua disponível para instalar somente a CLI:

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

O ZIP Windows usa um executável GNU x64 com validação parcial no Wine. O
assistente é compilado com MSVC no CI; `windows-installer.yml` testa sua instalação
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
O workflow `ci.yml` compila, testa e empacota em runners Linux e Windows.
Os artefatos ficam no próprio workflow. Não há publicação automática de releases,
instalador remoto ou pacote winget/Scoop ainda.

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

## Pacotes C++

Para projetos sem framework, use `cppm init meu-app`. Também há templates:

```sh
cppm init minha-api --template drogon
cppm init meu-wasm --template wasm
```

No Drogon, `run dev` inicia uma API local com `/health` e `run test` valida uma
resposta HTTP real. O Conan instala Drogon 1.9.14 e suas dependências.

No WASM, instale o SDK Emscripten e configure-o uma vez com `emsdk activate latest`.
O cppm localiza o SDK automaticamente. `run dev` compila e serve
o HTML em localhost; `run build` gera os artefatos em
`.cppm/build/wasm-release/bin/`; `run test` usa Node.js. A porta do servidor pode
ser alterada com `cppm run dev -- --port 9090`. Dependências Conan nativas não
podem ser ligadas ao módulo WASM nesta versão.

Instale o [SDK Emscripten](https://emscripten.org/docs/getting_started/downloads.html)
e execute no seu projeto:

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
