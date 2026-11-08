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
    let details = match template {
        Template::App => "`cppm run dev` compiles and runs the application.",
        Template::Drogon => {
            "`cppm run dev` starts the API at http://127.0.0.1:8080.\n\nGET `/` and `/health` return JSON. Change the port with `cppm run dev -- --port 9090`.\n`cppm run test` starts a temporary server and checks its health endpoint. Python 3 is required for that test."
        }
        Template::Wasm => {
            "The complete cppm Windows setup installs Emscripten and its private Node.js automatically. If you installed cppm 0.4.1, rerun the current Windows setup and open a new terminal; keep this project.\nFor other installations, install the [Emscripten SDK](https://emscripten.org/docs/getting_started/downloads.html) and configure it once with `emsdk activate latest`.\nCppm automatically finds SDKs selected with `CPPM_EMSDK` or `EMSDK`, tools in PATH, SDKs in `~/emsdk` or `~/.local/share/cppm/emsdk`, and the SDK in a local cppm checkout.\nYou can run `cppm run dev` in a fresh terminal without sourcing an environment script.\nTo select another SDK, set `CPPM_EMSDK` to its directory. Existing manually activated environments also work.\nRun `cppm doctor` in this directory to see the selected SDK and check Emscripten, Node.js and Python 3 by compiling a WebAssembly probe.\n\n`cppm run dev` compiles and serves the application at http://127.0.0.1:8080.\nThe command prints the HTML URL. Change the port with `cppm run dev -- --port 9090`.\n`cppm run build` generates HTML, JS and WASM in `.cppm/build/wasm-release/bin/`.\n`cppm run test` runs the generated JS/WASM with Node.js.\nPython 3 is required for the development server. External Conan dependencies for WASM are not supported yet."
        }
    };
    files.insert(
        "README.md",
        format!(
            "# {name}\n\n{details}\n\n```sh\ncppm run dev\ncppm run build\ncppm run test\n```\n\nUse `cppm watch` to rebuild and restart development when you save source/configuration files.\n`cppm watch test` reruns tests after edits. Press Ctrl+C to stop.\nIn WASM projects, refresh the browser after rebuilding.\nArguments can be forwarded, for example `cppm watch dev -- --port 9090` for Drogon/WASM.\n\nRepeated builds reuse unchanged dependencies and CMake configuration.\nUse `cppm build --fresh` to force dependency preparation and a new CMake cache.\n`cppm clean --dry-run` previews build cleanup; `cppm clean` removes build artifacts while keeping dependencies and sources.\n\nVS Code: Ctrl+Shift+B builds the project. Use `Tasks: Run Task` -> `cppm: run` to run it or `cppm: watch` to develop with automatic restarts.\nOpen the project folder, or run `cppm vscode --workspace ..` if the editor opened its parent.\n"
        ),
    );
    Scaffold { manifest, files }
}

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
        "$<TARGET_FILE_DIR:${PROJECT_NAME}>/${PROJECT_NAME}.js")
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
