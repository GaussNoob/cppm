cmake_minimum_required(VERSION 3.24)
project(hello LANGUAGES CXX)

include("${CMAKE_CURRENT_SOURCE_DIR}/.cppm/dependencies.cmake")

file(GLOB_RECURSE app_sources CONFIGURE_DEPENDS "src/*.cpp")
add_executable(${PROJECT_NAME} ${app_sources})
target_compile_features(${PROJECT_NAME} PRIVATE cxx_std_${CPPM_CXX_STANDARD})
set_target_properties(${PROJECT_NAME} PROPERTIES CXX_EXTENSIONS OFF)
target_include_directories(${PROJECT_NAME} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PROJECT_NAME} PRIVATE cppm_dependencies)

include(CTest)
if(BUILD_TESTING)
    add_test(NAME app_smoke COMMAND ${PROJECT_NAME})
endif()
