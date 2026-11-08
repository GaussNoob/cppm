
# The recipe's helper can find a different protoc on the system PATH first.
# Prefer the compiler bundled with the selected native Protobuf runtime.
if(TARGET protobuf::protoc)
    set(CPPM_PROTOBUF_BIN_DIRS)
    string(TOUPPER "${CMAKE_BUILD_TYPE}" CPPM_PROTOBUF_CONFIG)
    foreach(CPPM_CONFIG ${CPPM_PROTOBUF_CONFIG} DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
        if(DEFINED protobuf_PACKAGE_FOLDER_${CPPM_CONFIG})
            list(APPEND CPPM_PROTOBUF_BIN_DIRS "${protobuf_PACKAGE_FOLDER_${CPPM_CONFIG}}/bin")
        endif()
    endforeach()
    find_program(CPPM_PROTOC_PROGRAM NAMES protoc
        PATHS ${CPPM_PROTOBUF_BIN_DIRS}
        NO_DEFAULT_PATH)
    if(CPPM_PROTOC_PROGRAM)
        set_property(TARGET protobuf::protoc PROPERTY IMPORTED_LOCATION "${CPPM_PROTOC_PROGRAM}")
        set(Protobuf_PROTOC_EXECUTABLE "${CPPM_PROTOC_PROGRAM}" CACHE FILEPATH "The selected cppm Protobuf compiler" FORCE)
    endif()
endif()
