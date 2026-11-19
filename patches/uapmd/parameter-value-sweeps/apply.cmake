find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(value_sweeps_dir "${CMAKE_CURRENT_LIST_DIR}")
set(value_sweeps_host "${REPL_PROJECT_ROOT}/target/shim/generated/clap-note-dialect/HostClasses.cpp")
set(value_sweeps_timeline "${UAPMD_DIR}/source/uapmd-engine/src/sequencer/TimelineFacadePlugins.cpp")
set(value_sweeps_output "${REPL_PROJECT_ROOT}/target/shim/parameter-value-sweeps")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${value_sweeps_host}" "${value_sweeps_timeline}" "${value_sweeps_dir}/apply.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" -B "${value_sweeps_dir}/apply.py"
    "${value_sweeps_host}" "${value_sweeps_timeline}" "${value_sweeps_output}" "${UAPMD_DIR}"
    RESULT_VARIABLE value_sweeps_result ERROR_VARIABLE value_sweeps_error)
if(NOT value_sweeps_result EQUAL 0)
    message(FATAL_ERROR "${value_sweeps_error}")
endif()

function(value_sweeps_swap target input output)
    get_target_property(value_sweeps_before ${target} SOURCES)
    set(value_sweeps_sources "")
    set(value_sweeps_found 0)
    foreach(value_sweeps_source IN LISTS value_sweeps_before)
        if(value_sweeps_source STREQUAL input)
            list(APPEND value_sweeps_sources "${output}")
            math(EXPR value_sweeps_found "${value_sweeps_found} + 1")
        else()
            list(APPEND value_sweeps_sources "${value_sweeps_source}")
        endif()
    endforeach()
    if(NOT value_sweeps_found EQUAL 1)
        execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
            OUTPUT_VARIABLE value_sweeps_revision OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
        message(FATAL_ERROR "[parameter-value-sweeps] target: ${input}; expected one ${target} source, found ${value_sweeps_found}; UAPMD revision: ${value_sweeps_revision}; instructions: ${value_sweeps_dir}/README.md")
    endif()
    set_property(TARGET ${target} PROPERTY SOURCES "${value_sweeps_sources}")
endfunction()

value_sweeps_swap(remidy "${value_sweeps_host}" "${value_sweeps_output}/HostClasses.cpp")
value_sweeps_swap(uapmd-engine "src/sequencer/TimelineFacadePlugins.cpp" "${value_sweeps_output}/TimelineFacadePlugins.cpp")
# The copy includes TimelineFacadeImpl.hpp relative to the original source.
target_include_directories(uapmd-engine PRIVATE "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
