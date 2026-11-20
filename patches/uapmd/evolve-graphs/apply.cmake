find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(evolve_graphs_input "${REPL_PROJECT_ROOT}/target/shim/dag-builtins/SequencerEngine.cpp")
set(evolve_graphs_output "${REPL_PROJECT_ROOT}/target/shim/evolve-graphs/SequencerEngine.cpp")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE evolve_graphs_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${evolve_graphs_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/graphs.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${evolve_graphs_input}" "${evolve_graphs_output}" "${evolve_graphs_revision}"
    RESULT_VARIABLE evolve_graphs_result ERROR_VARIABLE evolve_graphs_error)
get_target_property(evolve_graphs_sources uapmd-engine SOURCES)
list(FIND evolve_graphs_sources "${evolve_graphs_input}" evolve_graphs_position)
if(NOT evolve_graphs_result EQUAL 0 OR evolve_graphs_position EQUAL -1)
    message(FATAL_ERROR "evolve-graphs: ${evolve_graphs_input}: ${evolve_graphs_error}; source index ${evolve_graphs_position}; upstream ${evolve_graphs_revision}; see ${CMAKE_CURRENT_LIST_DIR}/integration.md")
endif()
list(REMOVE_AT evolve_graphs_sources ${evolve_graphs_position})
list(INSERT evolve_graphs_sources ${evolve_graphs_position} "${evolve_graphs_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${evolve_graphs_sources}")
add_executable(evolve_graphs_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
