cmake_minimum_required(VERSION 3.24)
project(cppm_obfuscator LANGUAGES C CXX)
find_package(LLVM REQUIRED CONFIG)
if(LLVM_VERSION_MAJOR LESS 18 OR LLVM_VERSION_MAJOR GREATER 23)
    message(FATAL_ERROR "cppm's bundled obfuscator requires LLVM 18 through 23")
endif()
if(WIN32)
    message(FATAL_ERROR "The bundled plugin currently supports Linux and macOS LLVM hosts; use --obf-basic or a compatible external plugin on Windows")
endif()

add_custom_command(
    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/string/decode.h"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/string"
    COMMAND "${CMAKE_C_COMPILER}" -Os -g0 -emit-llvm -c
        "${CMAKE_CURRENT_SOURCE_DIR}/string/decode.c"
        -o "${CMAKE_CURRENT_BINARY_DIR}/string/decode.bc"
    COMMAND "${CMAKE_COMMAND}"
        "-DINPUT=${CMAKE_CURRENT_BINARY_DIR}/string/decode.bc"
        "-DOUTPUT=${CMAKE_CURRENT_BINARY_DIR}/string/decode.h"
        -P "${CMAKE_CURRENT_SOURCE_DIR}/embed-bitcode.cmake"
    DEPENDS string/decode.c embed-bitcode.cmake
    VERBATIM)
add_library(CppmObfuscator MODULE
    Plugin.cpp utils/Utils.cpp utils/CryptoUtils.cpp
    bogus/BogusControlFlow.cpp flattening/Flattening.cpp
    split/SplitBasicBlocks.cpp substitution/Substitution.cpp
    string/StringObfuscation.cpp "${CMAKE_CURRENT_BINARY_DIR}/string/decode.h")
target_compile_features(CppmObfuscator PRIVATE cxx_std_17)
target_include_directories(CppmObfuscator SYSTEM PRIVATE ${LLVM_INCLUDE_DIRS})
target_include_directories(CppmObfuscator PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_BINARY_DIR}")
separate_arguments(llvm_definitions NATIVE_COMMAND "${LLVM_DEFINITIONS}")
target_compile_options(CppmObfuscator PRIVATE ${llvm_definitions} -Wno-deprecated-declarations)
if(NOT LLVM_ENABLE_RTTI)
    target_compile_options(CppmObfuscator PRIVATE -fno-rtti)
endif()
if(APPLE)
    target_link_options(CppmObfuscator PRIVATE "LINKER:-undefined,dynamic_lookup")
endif()
# Resolve LLVM symbols from the matching Clang host, never a second static LLVM.
set_target_properties(CppmObfuscator PROPERTIES PREFIX "" LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
