use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env_remove("CPPM_SCRIPT_STACK")
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
        .prefix("cppm science ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "science-check"]);
    run(root, &["install", package]);
    fs::write(root.join("src/main.cpp"), source).unwrap();
    let locked = fs::read(root.join("cppm.lock")).unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
    run(root, &["test", "--offline", "--frozen-lockfile"]);
    assert_eq!(locked, fs::read(root.join("cppm.lock")).unwrap());
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn eigen_solves_a_matrix_operation() {
    check(
        "Eigen",
        "#include <Eigen/Dense>\nint main() { Eigen::Matrix2d matrix; matrix << 1, 2, 3, 4; return matrix.determinant() == -2.0 ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn glm_computes_a_vector_product() {
    check(
        "glm",
        "#include <glm/glm.hpp>\nint main() { return glm::dot(glm::vec3(1, 2, 3), glm::vec3(4, 5, 6)) == 32.0f ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn onetbb_executes_parallel_work() {
    check(
        "oneTBB",
        "#include <oneapi/tbb/parallel_for.h>\n#include <atomic>\nint main() { std::atomic<int> count{0}; oneapi::tbb::parallel_for(0, 42, [&](int) { ++count; }); return count == 42 ? 0 : 1; }\n",
    );
}

#[test]
#[ignore = "requires Conan, network access and OpenCV's platform prerequisites"]
fn opencv_transforms_an_image_without_opening_a_window() {
    check(
        "OpenCV",
        r#"#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <opencv2/videoio.hpp>
#include <opencv2/videoio/registry.hpp>
int main() {
#ifdef __linux__
    // Backend availability needs no camera and catches installs that process
    // images correctly but cannot open an ordinary Linux webcam.
    if (!cv::videoio_registry::hasBackend(cv::CAP_V4L2)) return 2;
#endif
    cv::Mat image(2, 2, CV_8UC3, cv::Scalar(0, 0, 255));
    cv::Mat gray;
    cv::cvtColor(image, gray, cv::COLOR_BGR2GRAY);
    return gray.rows == 2 && gray.cols == 2 && gray.channels() == 1 ? 0 : 1;
}
"#,
    );
}

#[test]
#[ignore = "requires Conan, network access and a native C++ toolchain"]
fn protobuf_serializes_and_parses_a_real_message() {
    check(
        "Protobuf",
        "#include <google/protobuf/struct.pb.h>\n#include <string>\nint main() { google::protobuf::Struct value; (*value.mutable_fields())[\"answer\"].set_number_value(42); std::string bytes; if (!value.SerializeToString(&bytes)) return 1; google::protobuf::Struct decoded; return decoded.ParseFromString(bytes) && decoded.fields().at(\"answer\").number_value() == 42 ? 0 : 2; }\n",
    );
}

#[test]
#[ignore = "requires the Conan Protobuf package in the local cache and a native C++ toolchain"]
fn protobuf_generates_cpp_from_a_schema() {
    let temp = tempfile::Builder::new()
        .prefix("cppm protobuf ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "schema-check"]);
    run(root, &["install", "protobuf", "--offline"]);
    fs::create_dir(root.join("proto")).unwrap();
    fs::write(
        root.join("proto/person.proto"),
        "syntax = \"proto3\";\nmessage Person { string name = 1; }\n",
    )
    .unwrap();
    let mut cmake = fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    cmake.push_str(
        r#"
add_custom_command(
    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/person.pb.cc" "${CMAKE_CURRENT_BINARY_DIR}/person.pb.h"
    COMMAND protobuf::protoc
    ARGS "--cpp_out=${CMAKE_CURRENT_BINARY_DIR}"
         "--proto_path=${CMAKE_CURRENT_SOURCE_DIR}/proto"
         "${CMAKE_CURRENT_SOURCE_DIR}/proto/person.proto"
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/proto/person.proto"
    VERBATIM)
target_sources(schema-check PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/person.pb.cc")
target_include_directories(schema-check PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
"#,
    );
    fs::write(root.join("CMakeLists.txt"), cmake).unwrap();
    fs::write(root.join("src/main.cpp"), "#include <person.pb.h>\n#include <string>\nint main() { Person person; person.set_name(\"cppm\"); std::string bytes; if (!person.SerializeToString(&bytes)) return 1; Person decoded; return decoded.ParseFromString(bytes) && decoded.name() == \"cppm\" ? 0 : 2; }\n").unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
    run(root, &["test", "--offline", "--frozen-lockfile"]);
}
