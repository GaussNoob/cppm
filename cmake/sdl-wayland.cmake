# SDL 3.4 can discover system libdecor while building its static Wayland backend.
# The Conan recipe does not propagate that optional link dependency to consumers.
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(CPPM_SDL_LIBDECOR QUIET IMPORTED_TARGET libdecor-0)
    if(TARGET PkgConfig::CPPM_SDL_LIBDECOR)
        target_link_libraries(cppm_dependencies INTERFACE PkgConfig::CPPM_SDL_LIBDECOR)
    endif()
endif()
