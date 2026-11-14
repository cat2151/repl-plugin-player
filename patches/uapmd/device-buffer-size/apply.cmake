find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(device_buffer_input "${UAPMD_DIR}/source/uapmd-engine/src/devices/MiniAudioIODevice.cpp")
set(device_buffer_output "${REPL_PROJECT_ROOT}/target/shim/device-buffer-size/MiniAudioIODevice.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${device_buffer_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${device_buffer_input}" "${device_buffer_output}" "${UAPMD_DIR}"
    RESULT_VARIABLE device_buffer_result ERROR_VARIABLE device_buffer_error)
if(NOT device_buffer_result EQUAL 0)
    message(FATAL_ERROR "${device_buffer_error}")
endif()
get_target_property(device_buffer_sources uapmd-engine SOURCES)
list(FIND device_buffer_sources "src/devices/MiniAudioIODevice.cpp" device_buffer_position)
if(device_buffer_position EQUAL -1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE device_buffer_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
    message(FATAL_ERROR "device-buffer-size: ${device_buffer_input}: target source missing; upstream ${device_buffer_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT device_buffer_sources ${device_buffer_position})
list(INSERT device_buffer_sources ${device_buffer_position} "${device_buffer_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${device_buffer_sources}")
target_include_directories(uapmd-engine PRIVATE "${UAPMD_DIR}/source/uapmd-engine/src/devices")
