# {name}

Aplicativo desktop com interface gráfica moderna em C++, usando
[Raven](https://github.com/oscar-soirey/Raven), criado com `cppm init --template raven`.

```sh
cppm run dev      # compila e abre a janela
cppm run test     # valida ui/main.xml sem abrir janela (bom para CI)
cppm run build    # Release
```

O primeiro build baixa o Raven 0.2.3 (cerca de 14 MB, uma vez só, em `.cppm/vendor/`)
e o compila junto com o projeto em menos de um minuto. Os seguintes são incrementais.

## Preparar o ambiente

- **Linux:** headers do X11 e um driver Vulkan
  (`sudo apt install libx11-dev libvulkan1`, `sudo pacman -S libx11 vulkan-icd-loader`,
  `sudo dnf install libX11-devel vulkan-loader`). Wayland é suportado.
- **Windows:** o Visual Studio Build Tools já basta; o driver de vídeo traz o Vulkan.
- O primeiro build precisa de internet e de `git` (o Raven baixa os headers do Vulkan
  durante a configuração, se o Vulkan SDK não estiver instalado).

## Estrutura

```text
{name}/
  src/main.cpp      # lógica: inicia o Raven, liga ações a funções C++
  ui/main.xml       # a interface: janela, layouts, botões, textos
  ui/strings.csv    # traduções (pt, en); o idioma do sistema é escolhido sozinho
```

## Como funciona

A interface fica em XML e a lógica em C++. Elas se conectam por **nomes**:

- `OnClicked="app.clicked"` no XML chama a função registrada com
  `Raction_bind("app.clicked", ...)` no C++;
- `Name="status"` no XML permite achar o widget no C++ com
  `Rwidget_find(g_window, "status")`;
- `Text="@app.title"` busca o texto `app.title` em `ui/strings.csv`.

**Edição ao vivo:** com o app aberto (`cppm run dev`), edite e salve `ui/main.xml`
— a janela muda na hora, sem recompilar. Só alterações em `.cpp` precisam de rebuild.

## Próximos passos

### 1. Montar a tela

Layouts: `VBox` (vertical), `HBox` (horizontal), `Grid`, `ScrollBox`, `TabView`,
`Splitter`. Controles: `Label`, `Button`, `TextInput`, `Checkbox`, `Slider`,
`ComboBox`, `ListView`, `ProgressBar`, `Image`, `MenuBar`.

```xml
<Window Title="Cadastro" Width="520" Height="420">
	<VBox Spacing="10" Slot.Padding="24">
		<Label Text="Nome" />
		<TextInput Name="nome" Placeholder="Digite seu nome" OnCommitted="form.salvar" />
		<Checkbox Name="aceito" Text="Aceito os termos" Style="Switch" />
		<Slider Name="idade" Min="0" Max="100" Value="30" Step="1" />
		<ComboBox Name="plano" Items="Grátis|Pro|Empresa" Selected="0" />
		<HBox Spacing="8">
			<Button Text="Salvar" Style="Primary" OnClicked="form.salvar" />
			<Button Text="Tema claro/escuro" Style="Flat" OnClicked="theme.toggle" />
		</HBox>
	</VBox>
</Window>
```

`Slot.Fill="1"` faz um item ocupar o espaço livre; `Slot.HAlign="Left"` alinha.
Algumas ações já vêm prontas, sem C++: `theme.toggle`, `app.quit`, `window.close`,
`show:nome`, `hide:nome`, `toggle:nome`, `open:https://...`.

### 2. Ler o que o usuário digitou

Para o formulário acima, em `src/main.cpp`, antes de `Rwindow_create_from_xml_file`:

```cpp
Raction_bind("form.salvar", [](Rid, const char*, void*) {
    const std::string nome = Rtext_input_get_text(Rwidget_find(g_window, "nome"));
    const bool aceito = Rcheckbox_is_checked(Rwidget_find(g_window, "aceito"));
    const int idade = static_cast<int>(Rslider_get_value(Rwidget_find(g_window, "idade")));
    const int plano = Rcombo_box_get_selected(Rwidget_find(g_window, "plano"));

    if (nome.empty() || !aceito) {
        Rmessage_box(g_window, "Atenção", "Preencha o nome e aceite os termos.", R_MESSAGE_OK);
        return;
    }
    std::cout << nome << ", " << idade << " anos, plano " << plano << '\n';
}, nullptr);
```

Adicione `#include <string>` no topo. Se `Rwidget_find` não achar o nome, retorna
`R_INVALID_ID`; confira o `Name` no XML.

### 3. Listas que mudam

```xml
<HBox Spacing="8">
	<TextInput Name="nova" Placeholder="Nova tarefa" OnCommitted="tarefa.adicionar" Slot.Fill="1" />
	<Button Text="Adicionar" Style="Primary" OnClicked="tarefa.adicionar" />
</HBox>
<ListView Name="tarefas" Slot.Fill="1" />
```

```cpp
Raction_bind("tarefa.adicionar", [](Rid, const char*, void*) {
    const Rid campo = Rwidget_find(g_window, "nova");
    const std::string texto = Rtext_input_get_text(campo);
    if (texto.empty()) return;
    Rlist_view_add_item(Rwidget_find(g_window, "tarefas"), texto.c_str());
    Rtext_input_set_text(campo, "");
}, nullptr);
```

### 4. Visual e temas

```cpp
Rlook_set(R_LOOK_RAVEN_GLASS_DARK);   // antes de criar a janela
```

Outras opções: `R_LOOK_RAVEN_DARK`, `R_LOOK_RAVEN_LIGHT`, `R_LOOK_WINDOWS_DARK`,
`R_LOOK_MACOS_LIGHT`, `R_LOOK_GNOME_DARK`, `R_LOOK_NATIVE_DARK`... No XML, qualquer
widget aceita `Color.Accent="#E5484D"`, `Metric.CornerRadius="12"`,
`Material="FrostedGlass"` e animações de entrada como `Appear="FlyUp"`.

### 5. Traduções

Cada linha de `ui/strings.csv` é uma chave com uma coluna por idioma. No XML use
`Text="@chave"`; no C++, `Rtr("chave")` ou `Rtr_plural("chave", n)` (com `.one` e
`.other`). A ação pronta `language:en` troca o idioma em tempo real.

### 6. Separar a lógica

Cresceu? Coloque a lógica em arquivos próprios (`src/tarefas.cpp`, `include/tarefas.h`)
e deixe no `main.cpp` só a ligação entre ações e funções. Bibliotecas comuns
funcionam junto com o Raven: `cppm add json`, `cppm add sqlite3`, `cppm add fmt`.

### 7. Testes

`cppm run test` executa o app com `--check`: carrega `ui/main.xml` sem abrir janela
(backend `R_BACKEND_NULL`) e falha se o XML tiver erro ou se o widget `status` sumir.
Ao renomear widgets, atualize a verificação no final do `main()`.

### 8. Distribuir

`cppm run build` gera o executável em `.cppm/build/release/bin/`. O caminho da pasta
`ui/` é fixado na compilação (`APP_UI_DIR` no `CMakeLists.txt`). Para entregar a outra
máquina, copie `ui/` para junto do executável e compile com
`-DAPP_UI_DIR="ui"`, ou embuta o XML no binário com `Rwindow_create_from_xml(texto, tamanho)`.

Mais sobre o Raven: [referência do XML](https://github.com/oscar-soirey/Raven/blob/v0.2.3/docs/XML.md),
[temas](https://github.com/oscar-soirey/Raven/blob/v0.2.3/docs/THEMES.md) e o
designer visual `raven_designer`, que vem nos
[pacotes de release](https://github.com/oscar-soirey/Raven/releases/tag/v0.2.3).

