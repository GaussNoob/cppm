use crate::{
    editor,
    manifest::{Dependency, Manifest, Script, Target},
};
use clap::ValueEnum;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Template {
    #[default]
    App,
    #[value(alias = "dragon")]
    Drogon,
    #[value(alias = "webassembly")]
    Wasm,
    #[value(alias = "gui")]
    Raven,
}

pub struct Scaffold {
    pub manifest: Manifest,
    pub files: BTreeMap<&'static str, String>,
}

pub fn scaffold(name: String, template: Template) -> Scaffold {
    let mut manifest = Manifest::new(name.clone());
    let mut files = BTreeMap::new();
    let (main, test, extra) = match template {
        Template::App => (
            APP,
            "    add_test(NAME app_smoke COMMAND ${PROJECT_NAME})\n",
            "",
        ),
        Template::Drogon => {
            manifest
                .dependencies
                .insert("drogon".into(), Dependency::Version("1.9.14".into()));
            files.insert("tests/smoke.py", DROGON_SMOKE.into());
            (DROGON, DROGON_TEST, "")
        }
        Template::Wasm => {
            manifest.package.target = Target::Wasm;
            manifest.scripts.insert(
                "dev".into(),
                Script::Command(vec!["cppm".into(), "serve".into()]),
            );
            files.insert("web/shell.html", WASM_SHELL.into());
            (APP, WASM_TEST, WASM_CMAKE)
        }
        Template::Raven => {
            files.insert("ui/main.xml", RAVEN_XML.replace("{name}", &name));
            files.insert("ui/strings.csv", RAVEN_STRINGS.into());
            (RAVEN, RAVEN_TEST, RAVEN_CMAKE)
        }
    };
    files.insert("src/main.cpp", main.into());
    files.insert(
        ".vscode/tasks.json",
        format!(
            "{}\n",
            serde_json::to_string_pretty(&editor::vscode_tasks(
                "${workspaceFolder}",
                manifest.package.target
            ))
            .expect("VS Code task serialization cannot fail")
        ),
    );
    files.insert(
        ".vscode/settings.json",
        format!(
            "{}\n",
            serde_json::to_string_pretty(&editor::vscode_settings("${workspaceFolder}"))
                .expect("VS Code settings serialization cannot fail")
        ),
    );
    files.insert(
        "CMakeLists.txt",
        format!(
            r#"cmake_minimum_required(VERSION 3.24)
project({name} LANGUAGES CXX)

include("${{CMAKE_CURRENT_SOURCE_DIR}}/.cppm/dependencies.cmake")

file(GLOB_RECURSE app_sources CONFIGURE_DEPENDS "src/*.cpp")
add_executable(${{PROJECT_NAME}} ${{app_sources}})
target_compile_features(${{PROJECT_NAME}} PRIVATE cxx_std_${{CPPM_CXX_STANDARD}})
set_target_properties(${{PROJECT_NAME}} PROPERTIES CXX_EXTENSIONS OFF)
target_include_directories(${{PROJECT_NAME}} PRIVATE "${{CMAKE_CURRENT_SOURCE_DIR}}/include")
target_link_libraries(${{PROJECT_NAME}} PRIVATE cppm_dependencies)
{extra}
include(CTest)
if(BUILD_TESTING)
{test}endif()
"#
        ),
    );
    let guide = match template {
        Template::App => APP_GUIDE,
        Template::Drogon => DROGON_GUIDE,
        Template::Wasm => WASM_GUIDE,
        Template::Raven => RAVEN_GUIDE,
    };
    files.insert(
        "README.md",
        format!("{}{COMMON_GUIDE}", guide.replace("{name}", &name)),
    );
    Scaffold { manifest, files }
}

const APP_GUIDE: &str = include_str!("templates/app.md");
const DROGON_GUIDE: &str = include_str!("templates/drogon.md");
const WASM_GUIDE: &str = include_str!("templates/wasm.md");
const RAVEN_GUIDE: &str = include_str!("templates/raven.md");
const COMMON_GUIDE: &str = include_str!("templates/common.md");

const APP: &str =
    "#include <iostream>\n\nint main() {\n    std::cout << \"Hello from C++!\\n\";\n}\n";

const DROGON: &str = r#"#include <charconv>
#include <drogon/drogon.h>
#include <functional>
#include <iostream>
#include <string_view>

int main(int argc, char** argv) {
    int port = 8080;
    if (argc != 1) {
        if (argc != 3 || std::string_view(argv[1]) != "--port") {
            std::cerr << "Usage: app [--port 1-65535]\n";
            return 2;
        }
        const std::string_view value(argv[2]);
        const auto [end, error] = std::from_chars(value.data(), value.data() + value.size(), port);
        if (error != std::errc{} || end != value.data() + value.size() || port < 1 || port > 65535) {
            std::cerr << "Invalid port\n";
            return 2;
        }
    }
    const auto health = [](const drogon::HttpRequestPtr&,
                           std::function<void(const drogon::HttpResponsePtr&)>&& callback) {
        Json::Value body;
        body["status"] = "ok";
        body["framework"] = "drogon";
        callback(drogon::HttpResponse::newHttpJsonResponse(body));
    };
    // Add routes here, or move them into controllers in src/; README.md shows how.
    drogon::app().registerHandler("/", health, {drogon::Get});
    drogon::app().registerHandler("/health", health, {drogon::Get});
    std::cout << "Listening at http://127.0.0.1:" << port << std::endl;
    drogon::app().addListener("127.0.0.1", port).run();
}
"#;

const DROGON_TEST: &str = r#"    find_package(Python3 COMPONENTS Interpreter REQUIRED)
    add_test(NAME health COMMAND "${Python3_EXECUTABLE}"
        "${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke.py" "$<TARGET_FILE:${PROJECT_NAME}>")
    set_tests_properties(health PROPERTIES TIMEOUT 20)
"#;

const DROGON_SMOKE: &str = r#"import json
import socket
import subprocess
import sys
import time
from urllib.request import urlopen

with socket.socket() as reservation:
    reservation.bind(('127.0.0.1', 0))
    port = reservation.getsockname()[1]
server = subprocess.Popen([sys.argv[1], '--port', str(port)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
try:
    deadline = time.monotonic() + 10
    while True:
        if server.poll() is not None:
            raise RuntimeError('Server exited before becoming ready')
        try:
            with urlopen(f'http://127.0.0.1:{port}/health', timeout=1) as response:
                body = json.load(response)
            assert body == {'status': 'ok', 'framework': 'drogon'}, body
            print('GET /health: ok')
            break
        except OSError:
            if time.monotonic() >= deadline:
                raise RuntimeError('Server did not become ready')
            time.sleep(0.05)
finally:
    server.terminate()
    try:
        output, _ = server.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        server.kill()
        output, _ = server.communicate()
    if output:
        print(output.decode(errors='replace'))
"#;

const WASM_CMAKE: &str = r#"
if(NOT EMSCRIPTEN)
    message(FATAL_ERROR "Activate Emscripten and build this project with cppm build")
endif()
set_target_properties(${PROJECT_NAME} PROPERTIES SUFFIX ".html")
target_link_options(${PROJECT_NAME} PRIVATE
    "--shell-file" "${CMAKE_CURRENT_SOURCE_DIR}/web/shell.html"
    "-sENVIRONMENT=web,node")
"#;

const WASM_TEST: &str = r#"    if(DEFINED ENV{EMSDK_NODE} AND EXISTS "$ENV{EMSDK_NODE}")
        set(CPPM_NODE_EXECUTABLE "$ENV{EMSDK_NODE}")
    else()
        find_program(CPPM_NODE_EXECUTABLE NAMES node nodejs REQUIRED)
    endif()
    add_test(NAME wasm_smoke COMMAND "${CPPM_NODE_EXECUTABLE}"
        "$<TARGET_FILE_DIR:${PROJECT_NAME}>/${PROJECT_NAME}.js"
        WORKING_DIRECTORY "$<TARGET_FILE_DIR:${PROJECT_NAME}>")
"#;

const WASM_SHELL: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>C++ WebAssembly</title></head>
<body>
    <h1>C++ running in WebAssembly</h1>
    <pre id="output" aria-live="polite"></pre>
    <script>
        var Module = {
            print: function (text) { document.getElementById('output').textContent += text + '\n'; },
            printErr: function (text) { document.getElementById('output').textContent += text + '\n'; }
        };
    </script>
    {{{ SCRIPT }}}
</body>
</html>
"#;

const RAVEN: &str = r#"#include <Raven.h>

#include <cstring>
#include <iostream>

// Folder with main.xml and strings.csv, set in CMakeLists.txt.
#ifndef APP_UI_DIR
#define APP_UI_DIR "ui"
#endif

static Rid g_window = R_INVALID_ID;
static int g_clicks = 0;

int main(int argc, char** argv) {
    // --check loads the interface without opening a window; the tests use it.
    const bool check = argc > 1 && std::strcmp(argv[1], "--check") == 0;
    if (!Rinit(check ? R_BACKEND_NULL : R_BACKEND_VULKAN)) {
        std::cerr << "Raven could not start (is a Vulkan driver installed?)\n";
        return 1;
    }
    Rlocale_load_csv(APP_UI_DIR "/strings.csv");
    Rlocale_set_language(Rlocale_get_system_language());

    // OnClicked="app.clicked" in ui/main.xml calls this function.
    Raction_bind("app.clicked", [](Rid, const char*, void*) {
        g_clicks++;
        Rlabel_set_text(Rwidget_find(g_window, "status"), Rtr_plural("clicks", g_clicks));
    }, nullptr);

    g_window = Rwindow_create_from_xml_file(APP_UI_DIR "/main.xml");
    if (g_window == R_INVALID_ID) {
        std::cerr << "Could not load " APP_UI_DIR "/main.xml\n";
        Rshutdown();
        return 1;
    }
    if (check) {
        const bool ok = Rwidget_find(g_window, "status") != R_INVALID_ID;
        Rshutdown();
        std::cout << (ok ? "interface ok\n" : "widget 'status' not found\n");
        return ok ? 0 : 1;
    }
    while (Rdraw_frame()) {}
    Rshutdown();
}
"#;

const RAVEN_XML: &str = r#"<!-- Interface do app. Salve com o programa aberto e a janela se atualiza sozinha. -->
<Window Title="{name}" Width="480" Height="320">
	<VBox Spacing="12" Slot.Padding="24">
		<Label Text="@app.title" TextSize="24"/>
		<Label Name="status" Text="@app.none" Opacity="0.7"/>
		<Button Text="@app.button" Style="Primary" OnClicked="app.clicked" Slot.HAlign="Left"/>
	</VBox>
</Window>
"#;

const RAVEN_STRINGS: &str = "key,en,pt
app.title,Hello from Raven!,Olá do Raven!
app.none,You have not clicked yet.,Você ainda não clicou.
app.button,Click me,Clique aqui
clicks.one,{n} click,{n} clique
clicks.other,{n} clicks,{n} cliques
";

const RAVEN_CMAKE: &str = r#"
# Raven GUI (https://github.com/oscar-soirey/Raven), downloaded once into .cppm/vendor.
include(FetchContent)
set(CPPM_RAVEN_DIR "${CMAKE_CURRENT_SOURCE_DIR}/.cppm/vendor/raven-0.2.3")
if(EXISTS "${CPPM_RAVEN_DIR}/CMakeLists.txt")
    set(FETCHCONTENT_SOURCE_DIR_RAVEN "${CPPM_RAVEN_DIR}")
endif()
FetchContent_Declare(raven
    URL "https://github.com/oscar-soirey/Raven/archive/refs/tags/v0.2.3.tar.gz"
    URL_HASH SHA256=276d983bd8732437181eed5e955cedec36168f97e89a2013f61f7785935beb90
    SOURCE_DIR "${CPPM_RAVEN_DIR}")
FetchContent_MakeAvailable(raven)
target_link_libraries(${PROJECT_NAME} PRIVATE raven::raven)
target_compile_definitions(${PROJECT_NAME} PRIVATE APP_UI_DIR="${CMAKE_CURRENT_SOURCE_DIR}/ui")
"#;

const RAVEN_TEST: &str = "    add_test(NAME ui_check COMMAND ${PROJECT_NAME} --check)\n";
