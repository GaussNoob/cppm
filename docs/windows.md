# cppm no Windows

## Instalação completa

Na [última release](https://github.com/GaussNoob/cppm/releases/latest), baixe
`cppm-0.5.1-windows-x86_64-setup.exe` e abra o assistente. Ele prepara a CLI,
Python, Conan, CMake, Ninja, Emscripten, Node.js, MSVC e Windows SDK e registra o PATH do usuário.
Depois, basta abrir um terminal novo e executar `cppm doctor` ou criar um projeto.

O compilador existente é reutilizado quando MSVC e Windows SDK estão disponíveis.
Se faltarem, a instalação da Microsoft pede administrador e pode baixar vários
GB. Se precisar reiniciar, o assistente informa isso antes do primeiro uso.
O download exige Internet; cancelar ou perder a conexão não é tratado como sucesso.
Use o mesmo instalador para tentar novamente. Os detalhes ficam em
`%LOCALAPPDATA%\Programs\cppm\install.log` (ou na pasta escolhida no assistente).

Python e ferramentas de build ficam no diretório `tools` da instalação, em um
ambiente próprio gerenciado por [uv](https://docs.astral.sh/uv/guides/install-python/).
Não é preciso instalar Python, Rust, winget ou Chocolatey antes. O MSVC é instalado
pelo [bootstrapper oficial da Microsoft](https://learn.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio).
O SDK WebAssembly e seu Node privado ficam em `tools\emsdk` e são localizados
automaticamente pelo cppm, sem executar `emsdk_env.bat`. Projetos com requisitos
extras, como Qt ou GPU, ainda precisam dos componentes específicos dessas bibliotecas.

Para corrigir a ausência de Emscripten na instalação 0.4.1, rode o instalador
atual novamente e abra um terminal novo. O projeto existente é preservado:
execute `cppm doctor` e `cppm run dev` nele, sem repetir `cppm init`.

Para remover, use **Configurações → Aplicativos instalados → cppm**. As ferramentas
privadas são removidas; projetos, cache Conan e o Visual Studio compartilhado são
preservados. Entradas de PATH preexistentes também são preservadas.

Instalação silenciosa: `cppm-0.5.1-windows-x86_64-setup.exe /S`.
Ela ainda pode pedir UAC para instalar MSVC; código 3010 indica reinicialização
necessária, sem reiniciar automaticamente.

## Pacote portátil: somente a CLI

O pacote `cppm-0.5.1-windows-x86_64.zip` contém `cppm.exe`, os instaladores,
licença e os guias. Ele é destinado a Windows 10/11 x64 e não exige Rust ou
MinGW no computador do usuário. O compilador usado para gerar a CLI é
independente do compilador dos seus projetos C++.

Baixe também o arquivo `.sha256` se quiser conferir o download:

```powershell
Get-FileHash .\cppm-0.5.1-windows-x86_64.zip -Algorithm SHA256
Get-Content .\cppm-0.5.1-windows-x86_64.zip.sha256
Expand-Archive .\cppm-0.5.1-windows-x86_64.zip -DestinationPath .\cppm-extraido
cd .\cppm-extraido\cppm-0.5.1-windows-x86_64
powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

O instalador usa `%LOCALAPPDATA%\Programs\cppm\bin`, altera apenas o PATH do
usuário e não exige administrador. Abra um terminal novo e execute `cppm --version`.
Use `-InstallDir` para escolher outra pasta ou `-NoPath` para manter o PATH atual.
O desinstalador preserva entradas de PATH que já existiam e arquivos de terceiros.
Se preferir preparar as dependências a partir do ZIP, execute o mesmo script com
`-WithDependencies`; use `uninstall.ps1 -RemoveTools` para remover o ambiente privado.

## Preparar manualmente a compilação C++

Instale Visual Studio Build Tools com **Desktop development with C++**, incluindo
MSVC e Windows SDK. O cppm usa `vswhere` e `VsDevCmd.bat` para preparar os
subprocessos; você pode usar um PowerShell normal. Um Developer PowerShell já
configurado, ou um compilador explícito em `CC`/`CXX`, é respeitado.

Com Python 3 instalado, uma opção para as outras ferramentas é:

```powershell
py -m pip install --user "conan>=2.34,<3" "cmake>=3.24" ninja
```

Adicione a pasta de scripts dessa instalação ao PATH do usuário se ela ainda
não estiver disponível. Para localizar a pasta e usá-la no terminal atual:

```powershell
$toolScripts = py -c "import sysconfig; print(sysconfig.get_path('scripts', 'nt_user'))"
$env:Path = "$toolScripts;$env:Path"
cppm doctor
```

Também é possível instalar as ferramentas por uv ou pipx. Elas precisam estar
no PATH do terminal que vai chamar o cppm.

## Primeiro projeto

```powershell
cppm doctor
cppm init meu-app
cd .\meu-app
cppm install fmt json eigen
cppm run dev
cppm run test
cppm run build
```

`add` e `install` aceitam vários pacotes. Os aliases são normalizados e o Conan
instala o conjunto em uma única operação. Se um pacote falhar, o manifesto e o
lockfile anteriores são preservados. Use opções de mapeamento CMake explícito
com apenas um pacote por chamada.

No Prompt de Comando (CMD), use `dir` para listar arquivos. O `>` no fim do prompt
não faz parte dos comandos. Para mudar também a unidade, use `cd /d "C:\projetos\meu-app"`.

## SDL2 e SDL3

O comando `cppm add sdl` consulta a versão disponível no Conan, que pode ser SDL3.
Se seu código inclui `<SDL.h>`, selecione SDL2 com `cppm add sdl2` e rode
`cppm run dev` novamente. Para SDL3, use `cppm add sdl3` e `<SDL3/SDL.h>`.
As APIs das duas gerações diferem; consulte a
[migração oficial](https://wiki.libsdl.org/SDL3/README-migration).
Os exemplos [SDL2](../examples/sdl2/main.cpp) e [SDL3](../examples/sdl3/main.cpp)
compilam e executam sem abrir janela. O cppm usa os metadados CMake da receita
para configurar os includes e a linkagem; não é necessário adicionar caminhos
do cache Conan manualmente.

## DLLs, Qt e testes

No host MSVC/Windows, use `cppm build obf --obf-basic` para um Release separado
com IPO/LTO e redução de metadados. `cppm exec obf --obf-basic` e
`cppm test obf --obf-basic` usam o mesmo preset. O plugin incluído atualmente
exige um host LLVM Linux/macOS; plugins externos precisam de Clang GNU com
suporte à carga de plugins. Confira o [guia do preset obf](guide.md#build-protegido-e-plugins-llvm)
para os requisitos e limites. A interface interativa está disponível com
`cppm ui` também no Windows.

`cppm exec`, `cppm test` e os scripts padrão `dev`/`test` carregam o ambiente
de execução produzido pelo Conan para a configuração escolhida. Isso inclui o
PATH das DLLs e as variáveis das receitas, como `QT_PLUGIN_PATH`. Debug e Release
usam arquivos separados. O ambiente global e o registro do Windows não são
alterados por essa configuração de execução.

Esse ambiente serve ao desenvolvimento e aos testes. Para distribuir seu
aplicativo a outra máquina, empacote as DLLs, os plugins e os recursos exigidos
pelas bibliotecas; no Qt, use as ferramentas de implantação do framework.
O ZIP do cppm distribui a CLI, e não suas aplicações.

## Diagnóstico

```powershell
cppm doctor
cppm doctor --json
cppm doctor --json > diagnostico.json
```

O diagnóstico mostra as versões de CMake, Ninja e Conan, identifica o compilador
e compila e executa um programa C++ temporário com CMake/Ninja/CTest. Dentro de
um projeto, usa seu padrão C++. Todos os problemas encontrados aparecem no
relatório; falhas retornam status diferente de zero. Ele não instala ferramentas
nem consulta a rede. O JSON pode conter caminhos locais do computador.

| Sintoma | Verificação |
| --- | --- |
| `cppm` não encontrado | Abra um terminal novo após instalar e confira o PATH do usuário. |
| MSVC não encontrado | Instale a carga de trabalho C++, MSVC e o Windows SDK. Confira `CC`/`CXX` se houver configuração manual. |
| Conan, Ninja ou CMake ausente | Confira o PATH da instalação Python/uv/pipx. |
| DLL ausente ao abrir o `.exe` diretamente | Use `cppm exec` no desenvolvimento; o executável isolado precisa das bibliotecas em seu ambiente de execução. |
| Arquivos de receitas com caminho muito longo | Considere um `CONAN_HOME` mais curto, como `C:\conan`; um cache novo precisa instalar os pacotes novamente. |
| Build congelado falha após trocar de plataforma | Instale com o perfil Windows correspondente; o lockfile não substitui o compilador e o SDK. |

## O que foi validado

| Ambiente | Cobertura |
| --- | --- |
| Linux nativo | CLI, compilador, instalação em lote, rollback, bibliotecas compartilhadas em Debug/Release/CTest e Qt Core. |
| Windows GNU x64 no Wine | Testes de unidade/CLI, criação de projeto, catálogo, concorrência, lockfile, validação de argumentos e diagnóstico com ferramentas ausentes; os testes que precisam de ferramentas Windows ficam no CI nativo. |
| Windows nativo com MSVC e PowerShell | `windows-installer.yml`: instalação real, downloads das ferramentas e SDK, reinstalação, PATH novo, C++, CTest, SDL2, SDL3, WASM em Node/HTTP e desinstalação. `ci.yml` contém integrações adicionais com bibliotecas. |

O assistente usa uma CLI MSVC produzida no runner Windows. O ZIP portátil pode
usar `x86_64-pc-windows-gnu`, compilado em Linux e executado no Wine; o
`release.json` no pacote identifica o target. A verificação no Wine é parcial.
O workflow do assistente usa PowerShell e MSVC nativos, e o CI de bibliotecas
inclui verificações de DLLs com uma biblioteca compartilhada real.

Para repetir a verificação parcial no Linux com MinGW, Rust com o target GNU e
Wine instalados:

```sh
cargo build --release --locked --target x86_64-pc-windows-gnu
python scripts/test-windows-wine.py
python scripts/package.py --target x86_64-pc-windows-gnu
```

O teste com Wine deixa explicitamente de fora os testes que exigem compilação
C++ nativa no Windows ou Windows PowerShell. O `release.json` incluído nos
pacotes informa o target Rust utilizado.

Consulte também o [guia](guide.md), o [catálogo](packages.md) e a
[instalação](installation.md).
