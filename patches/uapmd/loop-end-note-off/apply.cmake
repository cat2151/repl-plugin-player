find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(loop_end_input "${UAPMD_DIR}/source/uapmd-engine/src/sequencer/TimelineFacadeAudio.cpp")
set(loop_end_output "${REPL_PROJECT_ROOT}/target/shim/loop-end-note-off/TimelineFacadeAudio.cpp")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE loop_end_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${loop_end_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${loop_end_input}" "${loop_end_output}"
    RESULT_VARIABLE loop_end_result ERROR_VARIABLE loop_end_error)
if(NOT loop_end_result EQUAL 0)
    message(FATAL_ERROR "loop-end-note-off: ${loop_end_input}: ${loop_end_error}; upstream ${loop_end_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
get_target_property(loop_end_sources uapmd-engine SOURCES)
list(FIND loop_end_sources "src/sequencer/TimelineFacadeAudio.cpp" loop_end_position)
if(loop_end_position EQUAL -1)
    message(FATAL_ERROR "loop-end-note-off: ${loop_end_input}: target source missing; upstream ${loop_end_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT loop_end_sources ${loop_end_position})
list(INSERT loop_end_sources ${loop_end_position} "${loop_end_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${loop_end_sources}")
# The copy includes TimelineFacadeImpl.hpp relative to the original source.
target_include_directories(uapmd-engine PRIVATE "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
add_executable(loop_end_note_off_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
target_include_directories(loop_end_note_off_regression PRIVATE
    "$<TARGET_PROPERTY:uapmd-engine,INCLUDE_DIRECTORIES>")
target_link_libraries(loop_end_note_off_regression PRIVATE uapmd-engine)
set_target_properties(loop_end_note_off_regression PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
