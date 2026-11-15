find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(freeze_load_input "${UAPMD_DIR}/source/tools/uapmd-app-model/src/AppModel.cpp")
set(freeze_load_output "${REPL_PROJECT_ROOT}/target/shim/freeze-project-load/AppModel.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${freeze_load_input}" "${CMAKE_CURRENT_LIST_DIR}/generate.py")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${freeze_load_input}" "${freeze_load_output}" "${UAPMD_DIR}"
    RESULT_VARIABLE freeze_load_result ERROR_VARIABLE freeze_load_error)
if(NOT freeze_load_result EQUAL 0)
    message(FATAL_ERROR "${freeze_load_error}")
endif()
get_target_property(freeze_load_sources uapmd-app-model SOURCES)
list(FIND freeze_load_sources "src/AppModel.cpp" freeze_load_position)
if(freeze_load_position EQUAL -1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE freeze_load_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
    message(FATAL_ERROR "freeze-project-load: ${freeze_load_input}: source entry missing; upstream ${freeze_load_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT freeze_load_sources ${freeze_load_position})
list(INSERT freeze_load_sources ${freeze_load_position} "${freeze_load_output}")
set_property(TARGET uapmd-app-model PROPERTY SOURCES "${freeze_load_sources}")
target_include_directories(uapmd-app-model PRIVATE "${UAPMD_DIR}/source/tools/uapmd-app-model/src")
