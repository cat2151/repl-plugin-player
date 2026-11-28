find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(preroll_start_input "${REPL_PROJECT_ROOT}/target/shim/frozen-loop-playback/TimelineFacadeAudio.cpp")
set(preroll_start_output "${REPL_PROJECT_ROOT}/target/shim/preroll-before-start/TimelineFacadeAudio.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${preroll_start_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${preroll_start_input}" "${preroll_start_output}" "${UAPMD_DIR}"
    RESULT_VARIABLE preroll_start_result ERROR_VARIABLE preroll_start_error)
if(NOT preroll_start_result EQUAL 0)
    message(FATAL_ERROR "${preroll_start_error}")
endif()
get_target_property(preroll_start_sources uapmd-engine SOURCES)
list(FIND preroll_start_sources "${preroll_start_input}" preroll_start_position)
if(preroll_start_position EQUAL -1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE preroll_start_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
    message(FATAL_ERROR "preroll-before-start: ${preroll_start_input}: target source missing; upstream ${preroll_start_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT preroll_start_sources ${preroll_start_position})
list(INSERT preroll_start_sources ${preroll_start_position} "${preroll_start_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${preroll_start_sources}")
add_executable(preroll_before_start_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
target_include_directories(preroll_before_start_regression PRIVATE
    "$<TARGET_PROPERTY:uapmd-engine,INCLUDE_DIRECTORIES>")
target_link_libraries(preroll_before_start_regression PRIVATE uapmd-engine)
set_target_properties(preroll_before_start_regression PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
