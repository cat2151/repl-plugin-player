find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE catalog_cache_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set(catalog_cache_input "${UAPMD_DIR}/source/uapmd-plugin-hosting/src/plugin-api")
set(catalog_cache_output "${REPL_PROJECT_ROOT}/target/shim/catalog-cache-startup")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${catalog_cache_input}/RemidyAudioPluginHost.hpp" "${catalog_cache_input}/RemidyAudioPluginHost.cpp"
    "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/catalog_cache.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${catalog_cache_input}" "${catalog_cache_output}" RESULT_VARIABLE catalog_cache_result ERROR_VARIABLE catalog_cache_error)
if(NOT catalog_cache_result EQUAL 0)
    message(FATAL_ERROR "catalog-cache-startup: ${catalog_cache_input}: ${catalog_cache_error}; upstream ${catalog_cache_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
get_target_property(catalog_cache_sources uapmd-plugin-hosting SOURCES)
list(FIND catalog_cache_sources "src/plugin-api/RemidyAudioPluginHost.cpp" catalog_cache_index)
if(catalog_cache_index EQUAL -1)
    message(FATAL_ERROR "catalog-cache-startup: uapmd-plugin-hosting target has no src/plugin-api/RemidyAudioPluginHost.cpp; upstream ${catalog_cache_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
list(REMOVE_AT catalog_cache_sources ${catalog_cache_index})
list(INSERT catalog_cache_sources ${catalog_cache_index} "${catalog_cache_output}/RemidyAudioPluginHost.cpp")
set_property(TARGET uapmd-plugin-hosting PROPERTY SOURCES "${catalog_cache_sources}")
