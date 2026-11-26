find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(playback_only_input "${REPL_PROJECT_ROOT}/target/shim/device-buffer-size/MiniAudioIODevice.cpp")
set(playback_only_output "${REPL_PROJECT_ROOT}/target/shim/playback-only-device/MiniAudioIODevice.cpp")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE playback_only_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${playback_only_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${playback_only_input}" "${playback_only_output}" "${playback_only_revision}"
    RESULT_VARIABLE playback_only_result ERROR_VARIABLE playback_only_error)
get_target_property(playback_only_sources uapmd-engine SOURCES)
list(FIND playback_only_sources "${playback_only_input}" playback_only_position)
if(NOT playback_only_result EQUAL 0 OR playback_only_position EQUAL -1)
    message(FATAL_ERROR "playback-only-device: ${playback_only_input}: ${playback_only_error}; source index ${playback_only_position} (include after device-buffer-size); upstream ${playback_only_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT playback_only_sources ${playback_only_position})
list(INSERT playback_only_sources ${playback_only_position} "${playback_only_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${playback_only_sources}")
