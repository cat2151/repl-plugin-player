find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(prefetch_input "${REPL_PROJECT_ROOT}/target/shim/freeze-monitor/FrozenTrackManager.cpp")
set(prefetch_output "${REPL_PROJECT_ROOT}/target/shim/frozen-loop-prefetch")
set(prefetch_upstream "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${prefetch_upstream}/FrozenTrackAudio.cpp" "${prefetch_upstream}/FrozenTrackAudio.hpp"
    "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${UAPMD_DIR}" "${prefetch_input}" "${prefetch_output}"
    RESULT_VARIABLE prefetch_result ERROR_VARIABLE prefetch_error)
if(NOT prefetch_result EQUAL 0)
    message(FATAL_ERROR "${prefetch_error}")
endif()
get_target_property(prefetch_sources uapmd-engine SOURCES)
foreach(original "src/sequencer/FrozenTrackAudio.cpp" "${prefetch_input}")
    list(FIND prefetch_sources "${original}" position)
    if(position EQUAL -1)
        execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
            OUTPUT_VARIABLE revision OUTPUT_STRIP_TRAILING_WHITESPACE)
        message(FATAL_ERROR "frozen-loop-prefetch: ${original}: source missing; upstream ${revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
    endif()
    get_filename_component(name "${original}" NAME)
    list(REMOVE_AT prefetch_sources ${position})
    list(INSERT prefetch_sources ${position} "${prefetch_output}/${name}")
endforeach()
set_property(TARGET uapmd-engine PROPERTY SOURCES "${prefetch_sources}")

add_executable(frozen_loop_prefetch_regression EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_LIST_DIR}/regression.cpp" "${prefetch_output}/FrozenTrackAudio.cpp")
get_target_property(prefetch_includes uapmd-engine INCLUDE_DIRECTORIES)
target_include_directories(frozen_loop_prefetch_regression PRIVATE "${prefetch_output}" ${prefetch_includes})
target_link_libraries(frozen_loop_prefetch_regression PRIVATE uapmd-engine)
set_target_properties(frozen_loop_prefetch_regression PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")

# The same sample test against the unmodified disk reader must reproduce the loss.
add_executable(frozen_loop_prefetch_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_LIST_DIR}/regression.cpp" "${prefetch_upstream}/FrozenTrackAudio.cpp")
target_compile_definitions(frozen_loop_prefetch_baseline PRIVATE PREFETCH_BASELINE)
target_include_directories(frozen_loop_prefetch_baseline PRIVATE "${prefetch_upstream}" ${prefetch_includes})
target_link_libraries(frozen_loop_prefetch_baseline PRIVATE uapmd-engine)
set_target_properties(frozen_loop_prefetch_baseline PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
