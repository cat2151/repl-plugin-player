find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(freeze_monitor_input "${REPL_PROJECT_ROOT}/target/shim/frozen-loop-playback/FrozenTrackManager.cpp")
set(freeze_monitor_output "${REPL_PROJECT_ROOT}/target/shim/freeze-monitor")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${UAPMD_DIR}/source/uapmd-engine/include/uapmd-engine/uapmd-engine.hpp"
    "${UAPMD_DIR}/source/uapmd-engine/include/uapmd-engine/detail/sequencer/FrozenTrackManager.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${UAPMD_DIR}" "${freeze_monitor_input}" "${freeze_monitor_output}"
    RESULT_VARIABLE freeze_monitor_result ERROR_VARIABLE freeze_monitor_error)
if(NOT freeze_monitor_result EQUAL 0)
    message(FATAL_ERROR "${freeze_monitor_error}")
endif()
get_target_property(freeze_monitor_sources uapmd-engine SOURCES)
list(FIND freeze_monitor_sources "${freeze_monitor_input}" freeze_monitor_position)
if(freeze_monitor_position EQUAL -1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE freeze_monitor_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
    message(FATAL_ERROR "freeze-monitor: ${freeze_monitor_input}: source entry missing; upstream ${freeze_monitor_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT freeze_monitor_sources ${freeze_monitor_position})
list(INSERT freeze_monitor_sources ${freeze_monitor_position} "${freeze_monitor_output}/FrozenTrackManager.cpp")
set_property(TARGET uapmd-engine PROPERTY SOURCES "${freeze_monitor_sources}")
# Every consumer must see the same PlaybackState definition.
target_include_directories(uapmd-engine BEFORE PUBLIC "${freeze_monitor_output}/include")
