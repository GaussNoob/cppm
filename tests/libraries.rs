use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check(package: &str, source: &str) {
    let temp = tempfile::Builder::new()
        .prefix("cppm library ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "library-check"]);
    run(root, &["install", package]);
    fs::write(root.join("src/main.cpp"), source).unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
    run(root, &["test", "--offline", "--frozen-lockfile"]);
}

#[test]
#[ignore = "requires Conan, network access and the native library's system prerequisites"]
fn sfml_links_without_opening_a_window() {
    check(
        "SFML",
        "#include <SFML/System.hpp>\nint main() { sf::Clock clock; return clock.getElapsedTime().asMicroseconds() < 0; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and the native library's system prerequisites"]
fn raylib_links_without_opening_a_window() {
    check(
        "raylib",
        "#include <raylib.h>\nint main() { return TextLength(\"cppm\") == 4 ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and the native library's system prerequisites"]
fn curl_alias_links_without_an_http_request() {
    check(
        "curl",
        "#include <curl/curl.h>\nint main() { return curl_version_info(CURLVERSION_NOW)->version_num > 0 ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and the native library's system prerequisites"]
fn sdl_links_without_initializing_video() {
    check(
        "sdl3",
        "#include <SDL3/SDL.h>\nint main() { return SDL_GetVersion() >= 3000000 ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_boost_filesystem_links_and_performs_a_real_operation() {
    check(
        "Boost",
        "#include <boost/filesystem.hpp>\nint main() { return boost::filesystem::exists(boost::filesystem::current_path()) ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_httplib_alias_links_and_registers_an_http_route() {
    check(
        "httplib",
        "#include <httplib.h>\nint main() { httplib::Server server; server.Get(\"/health\", [](const httplib::Request&, httplib::Response& response) { response.set_content(\"ok\", \"text/plain\"); }); return server.is_valid() ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_asio_executes_a_queued_handler() {
    check(
        "Asio",
        "#include <asio.hpp>\nint main() { asio::io_context io; bool called = false; asio::post(io, [&] { called = true; }); io.run(); return called ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_dear_imgui_alias_links_without_opening_a_window() {
    check(
        "dear-imgui",
        "#include <imgui.h>\nint main() { IMGUI_CHECKVERSION(); ImGuiContext* context = ImGui::CreateContext(); if (!context) return 1; ImGui::StyleColorsDark(); const bool valid = ImGui::GetCurrentContext() == context; ImGui::DestroyContext(context); return valid ? 0 : 2; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_catch2_executes_a_real_test_case() {
    check(
        "Catch2",
        "#include <catch2/catch_session.hpp>\n#include <catch2/catch_test_macros.hpp>\nTEST_CASE(\"the library is linked\") { REQUIRE(6 * 7 == 42); }\nint main(int argc, char** argv) { return Catch::Session().run(argc, argv); }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn standard_googletest_alias_executes_a_real_test_case() {
    check(
        "googletest",
        "#include <gtest/gtest.h>\nTEST(Cppm, LinkedLibrary) { EXPECT_EQ(6 * 7, 42); }\nint main(int argc, char** argv) { ::testing::InitGoogleTest(&argc, argv); return RUN_ALL_TESTS(); }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and Qt's platform development prerequisites"]
fn standard_qt_core_links_without_opening_a_window() {
    check(
        "Qt",
        "#include <QCoreApplication>\n#include <QString>\nint main(int argc, char** argv) { QCoreApplication app(argc, argv); return QString::number(42) == QStringLiteral(\"42\") ? 0 : 1; }\n",
    );
}
