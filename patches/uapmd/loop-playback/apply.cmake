find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(loop_source "${UAPMD_DIR}/source/uapmd-engine/src/sequencer/TimelineFacadeAudio.cpp")
set(loop_output "${REPL_PROJECT_ROOT}/target/shim/loop-playback/TimelineFacadeAudio.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${loop_source}" "${CMAKE_CURRENT_LIST_DIR}/apply.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/apply.py"
    "${UAPMD_DIR}" "${loop_output}" "${REPL_PROJECT_ROOT}/native/loop_playback.hpp"
    COMMAND_ERROR_IS_FATAL ANY)
get_target_property(loop_sources uapmd-engine SOURCES)
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE loop_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
list(FIND loop_sources "src/sequencer/TimelineFacadeAudio.cpp" loop_index)
if(loop_index EQUAL -1)
    message(FATAL_ERROR "loop-playback: ${loop_source}: target source missing; upstream ${loop_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_ITEM loop_sources "src/sequencer/TimelineFacadeAudio.cpp")
list(APPEND loop_sources "${loop_output}")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${loop_sources}")
target_include_directories(uapmd-engine PRIVATE "${UAPMD_DIR}/source/uapmd-engine/src/sequencer")
