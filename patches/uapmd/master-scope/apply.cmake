find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(master_scope_source "${UAPMD_DIR}/source/uapmd-engine/src/sequencer/RealtimeSequencer.cpp")
set(master_scope_output "${REPL_PROJECT_ROOT}/target/shim/master-scope/RealtimeSequencer.cpp")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE master_scope_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${master_scope_source}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${master_scope_source}" "${master_scope_output}" "${REPL_PROJECT_ROOT}/native/scope_capture.hpp" "${master_scope_revision}"
    RESULT_VARIABLE master_scope_result ERROR_VARIABLE master_scope_error)
get_target_property(master_scope_sources uapmd-engine SOURCES)
list(FIND master_scope_sources "src/sequencer/RealtimeSequencer.cpp" master_scope_position)
if(NOT master_scope_result EQUAL 0 OR master_scope_position EQUAL -1)
    message(FATAL_ERROR "master-scope: ${master_scope_source}: ${master_scope_error}; source index ${master_scope_position}; upstream ${master_scope_revision}; see ${CMAKE_CURRENT_LIST_DIR}/integration.md")
endif()
list(REMOVE_AT master_scope_sources ${master_scope_position})
list(INSERT master_scope_sources ${master_scope_position} "${master_scope_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${master_scope_sources}")
