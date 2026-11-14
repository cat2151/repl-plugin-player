find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(loop_end_input "${UAPMD_DIR}/source/uapmd-data/src/timeline")
set(loop_end_output "${REPL_PROJECT_ROOT}/target/shim/loop-end-note-off")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE loop_end_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${loop_end_input}/TimelineTrack.cpp" "${loop_end_input}/MidiClipSourceNode.cpp"
    "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/boundary.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${loop_end_input}" "${loop_end_output}"
    RESULT_VARIABLE loop_end_result ERROR_VARIABLE loop_end_error)
if(NOT loop_end_result EQUAL 0)
    message(FATAL_ERROR "loop-end-note-off: ${loop_end_input}/TimelineTrack.cpp or MidiClipSourceNode.cpp: ${loop_end_error}; upstream ${loop_end_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
get_target_property(loop_end_sources uapmd-data SOURCES)
foreach(name TimelineTrack MidiClipSourceNode)
    set(original "src/timeline/${name}.cpp")
    list(FIND loop_end_sources "${original}" position)
    if(position EQUAL -1)
        message(FATAL_ERROR "loop-end-note-off: ${original}: target source missing; upstream ${loop_end_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
    endif()
    list(REMOVE_AT loop_end_sources ${position})
    list(INSERT loop_end_sources ${position} "${loop_end_output}/${name}.cpp")
endforeach()
set_property(TARGET uapmd-data PROPERTY SOURCES "${loop_end_sources}")
add_executable(loop_end_note_off_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
get_target_property(loop_end_includes uapmd-data INCLUDE_DIRECTORIES)
target_include_directories(loop_end_note_off_regression PRIVATE ${loop_end_includes})
target_link_libraries(loop_end_note_off_regression PRIVATE uapmd-data)
set_target_properties(loop_end_note_off_regression PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
