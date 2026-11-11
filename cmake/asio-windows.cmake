# Asio (standalone or Boost) warns on every compilation unless the targeted
# Windows version is declared. Target Windows 10; set CPPM_WIN32_WINNT to
# another value, or to an empty string to define _WIN32_WINNT yourself.
if(WIN32)
    if(NOT DEFINED CPPM_WIN32_WINNT)
        set(CPPM_WIN32_WINNT 0x0A00)
    endif()
    if(CPPM_WIN32_WINNT)
        foreach(cppm_target IN ITEMS cppm_dependencies cppm_dev_dependencies)
            target_compile_definitions(${cppm_target} INTERFACE _WIN32_WINNT=${CPPM_WIN32_WINNT})
        endforeach()
    endif()
endif()
