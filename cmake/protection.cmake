# Injected by cppm after project(), without modifying the user's CMakeLists.txt.
# IPO probes and third-party subprojects must not inherit this hook.
if(NOT CMAKE_SOURCE_DIR STREQUAL CPPM_PROTECTED_ROOT OR
   NOT CMAKE_CURRENT_SOURCE_DIR STREQUAL CMAKE_SOURCE_DIR)
    return()
endif()
include_guard(DIRECTORY)

function(_cppm_apply_protection)
    set(app "${CPPM_PROTECTED_TARGET}")
    if(NOT TARGET "${app}")
        message(FATAL_ERROR "cppm obf: CMake executable target '${app}' must match package.name")
    endif()
    get_target_property(kind "${app}" TYPE)
    get_target_property(imported "${app}" IMPORTED)
    if(NOT kind STREQUAL "EXECUTABLE" OR imported)
        message(FATAL_ERROR "cppm obf: '${app}' must be a locally built executable")
    endif()
    if(CPPM_OBF_PLUGIN AND (NOT CMAKE_CXX_COMPILER_ID STREQUAL "Clang" OR
                           CMAKE_CXX_COMPILER_FRONTEND_VARIANT STREQUAL "MSVC"))
        message(FATAL_ERROR "cppm obf: --obf-plugin requires Clang with its GNU frontend; select a compatible compiler with CXX and a matching Conan profile")
    endif()

    include(CheckIPOSupported)
    check_ipo_supported(RESULT ipo_supported OUTPUT ipo_error LANGUAGES CXX)
    if(NOT ipo_supported)
        message(FATAL_ERROR "cppm obf requires working IPO/LTO support in the compiler and linker.\n${ipo_error}")
    endif()
    set_target_properties("${app}" PROPERTIES
        INTERPROCEDURAL_OPTIMIZATION TRUE
        INTERPROCEDURAL_OPTIMIZATION_RELEASE TRUE
        CXX_VISIBILITY_PRESET hidden
        VISIBILITY_INLINES_HIDDEN TRUE)

    if(MSVC)
        if(CMAKE_CXX_COMPILER_ID STREQUAL "MSVC")
            # cl.exe requires deterministic mode for /pathmap to take effect.
            target_compile_options("${app}" PRIVATE /experimental:deterministic)
        endif()
        target_compile_options("${app}" PRIVATE /O2 /Gy /Gw "/pathmap:${CPPM_PROTECTED_ROOT}=.")
        target_link_options("${app}" PRIVATE /DEBUG:NONE /OPT:REF /OPT:ICF /INCREMENTAL:NO)
    elseif(CMAKE_CXX_COMPILER_ID MATCHES "^(GNU|Clang|AppleClang)$")
        target_compile_options("${app}" PRIVATE
            -O3 -g0 -fno-ident -fno-record-gcc-switches
            "-ffile-prefix-map=${CPPM_PROTECTED_ROOT}=."
            -ffunction-sections -fdata-sections)
        if(APPLE)
            target_link_options("${app}" PRIVATE "LINKER:-S,-x,-dead_strip")
        else()
            target_link_options("${app}" PRIVATE "LINKER:--strip-all" "LINKER:--gc-sections")
        endif()
    else()
        message(FATAL_ERROR "cppm obf supports GCC, Clang and MSVC; compiler '${CMAKE_CXX_COMPILER_ID}' is unsupported")
    endif()

    if(CPPM_OBF_PLUGIN)
        target_compile_options("${app}" PRIVATE "$<$<COMPILE_LANGUAGE:CXX>:-fpass-plugin=${CPPM_OBF_PLUGIN}>")
        # A changed plugin at the same path must invalidate existing objects.
        target_compile_definitions("${app}" PRIVATE "CPPM_OBF_PLUGIN_DIGEST=${CPPM_OBF_PLUGIN_DIGEST}")
    endif()
    file(WRITE "${CMAKE_BINARY_DIR}/cppm-protection.applied" "applied\n")
endfunction()

# Apply after the user has declared the executable and its target properties.
cmake_language(DEFER CALL _cppm_apply_protection)
