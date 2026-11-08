
# libpq 17.11's Meson build can discover system GSSAPI without publishing
# that dependency in its Conan metadata. Complete the static link when present.
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_search_module(CPPM_LIBPQ_GSSAPI QUIET IMPORTED_TARGET
        krb5-gssapi heimdal-gssapi)
endif()
if(TARGET PkgConfig::CPPM_LIBPQ_GSSAPI)
    target_link_libraries(cppm_dependencies INTERFACE PkgConfig::CPPM_LIBPQ_GSSAPI)
else()
    find_library(CPPM_LIBPQ_GSSAPI_LIBRARY NAMES gssapi_krb5 gssapi)
    if(CPPM_LIBPQ_GSSAPI_LIBRARY)
        target_link_libraries(cppm_dependencies INTERFACE "${CPPM_LIBPQ_GSSAPI_LIBRARY}")
    endif()
endif()
