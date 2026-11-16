find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(evolution_graphs_input "${REPL_PROJECT_ROOT}/target/shim/dag-builtins/SequencerEngine.cpp")
set(evolution_graphs_workers "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
set(evolution_graphs_output "${REPL_PROJECT_ROOT}/target/shim/evolution-graphs/SequencerEngine.cpp")
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE evolution_graphs_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${evolution_graphs_workers}/AudioTrackWorkerPool.hpp" "${evolution_graphs_workers}/AudioWorkers.hpp" "${evolution_graphs_workers}/AudioWorkers.cpp"
    "${evolution_graphs_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/graphs.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${evolution_graphs_input}" "${evolution_graphs_output}" "${evolution_graphs_revision}" "${evolution_graphs_workers}"
    RESULT_VARIABLE evolution_graphs_result ERROR_VARIABLE evolution_graphs_error)
get_target_property(evolution_graphs_sources uapmd-engine SOURCES)
list(FIND evolution_graphs_sources "${evolution_graphs_input}" evolution_graphs_position)
if(NOT evolution_graphs_result EQUAL 0 OR evolution_graphs_position EQUAL -1)
    message(FATAL_ERROR "evolution-graphs: ${evolution_graphs_input}: ${evolution_graphs_error}; source index ${evolution_graphs_position}; upstream ${evolution_graphs_revision}; see ${CMAKE_CURRENT_LIST_DIR}/integration.md")
endif()
list(REMOVE_AT evolution_graphs_sources ${evolution_graphs_position})
list(INSERT evolution_graphs_sources ${evolution_graphs_position} "${evolution_graphs_output}")
list(FIND evolution_graphs_sources "src/sequencer/AudioWorkers.cpp" evolution_graphs_workers_position)
if(evolution_graphs_workers_position EQUAL -1)
    message(FATAL_ERROR "evolution-graphs: ${evolution_graphs_workers}/AudioWorkers.cpp: target source missing; upstream ${evolution_graphs_revision}; see ${CMAKE_CURRENT_LIST_DIR}/integration.md")
endif()
list(REMOVE_AT evolution_graphs_sources ${evolution_graphs_workers_position})
list(INSERT evolution_graphs_sources ${evolution_graphs_workers_position} "${REPL_PROJECT_ROOT}/target/shim/evolution-graphs/AudioWorkers.cpp")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${evolution_graphs_sources}")
add_executable(evolution_graphs_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
