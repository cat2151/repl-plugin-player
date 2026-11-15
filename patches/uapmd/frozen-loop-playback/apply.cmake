find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(frozen_loop_input "${UAPMD_DIR}/source/uapmd-engine/src/sequencer/FrozenTrackManager.cpp")
set(frozen_loop_output "${REPL_PROJECT_ROOT}/target/shim/frozen-loop-playback/FrozenTrackManager.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${frozen_loop_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/segments.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${frozen_loop_input}" "${frozen_loop_output}" "${UAPMD_DIR}"
    RESULT_VARIABLE frozen_loop_result ERROR_VARIABLE frozen_loop_error)
if(NOT frozen_loop_result EQUAL 0)
    message(FATAL_ERROR "${frozen_loop_error}")
endif()
get_target_property(frozen_loop_sources uapmd-engine SOURCES)
list(FIND frozen_loop_sources "src/sequencer/FrozenTrackManager.cpp" frozen_loop_position)
if(frozen_loop_position EQUAL -1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE frozen_loop_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
    message(FATAL_ERROR "frozen-loop-playback: ${frozen_loop_input}: target source missing; upstream ${frozen_loop_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT frozen_loop_sources ${frozen_loop_position})
list(INSERT frozen_loop_sources ${frozen_loop_position} "${frozen_loop_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${frozen_loop_sources}")
target_include_directories(uapmd-engine PRIVATE "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
add_executable(frozen_loop_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
set_target_properties(frozen_loop_regression PROPERTIES RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
