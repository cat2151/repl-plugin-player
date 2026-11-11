# Compose after clap-presets, which owns the complete generated CLAP directory.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_LIST_DIR}/generate.py" "${CMAKE_CURRENT_LIST_DIR}/metadata.hpp")
execute_process(COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/generate.py"
    "${clap_presets_output}" "${script_api_output}" RESULT_VARIABLE metadata_result ERROR_VARIABLE metadata_error)
if(NOT metadata_result EQUAL 0)
    message(FATAL_ERROR "parameter-metadata: ${clap_presets_output}/PluginInstanceCLAP.Parameters.cpp: ${metadata_error}; upstream ${clap_presets_revision}; see ${CMAKE_CURRENT_LIST_DIR}/README.md")
endif()
add_executable(parameter_metadata_regression EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/regression.cpp")
set_target_properties(parameter_metadata_regression PROPERTIES RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/out>")
