set(handle_dir "${CMAKE_CURRENT_LIST_DIR}")
set(handle_clap_header "${REPL_PROJECT_ROOT}/target/shim/generated/clap-note-dialect/PluginFormatCLAP.hpp")
set(handle_upstream_instance "${UAPMD_DIR}/source/uapmd-plugin-hosting/src/plugin-api/RemidyAudioPluginInstance.hpp")
set(handle_output "${REPL_PROJECT_ROOT}/target/shim/clap-plugin-handle")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${handle_clap_header}" "${handle_upstream_instance}"
    "${handle_dir}/clap.cpp.in" "${handle_dir}/instance.cpp.in")

# Stops unless `anchor` occurs exactly once in `file`.
function(handle_require_once file anchor)
    file(READ "${file}" text)
    string(REPLACE "${anchor}" "" without "${text}")
    string(LENGTH "${text}" with_length)
    string(LENGTH "${without}" without_length)
    string(LENGTH "${anchor}" anchor_length)
    math(EXPR count "(${with_length} - ${without_length}) / ${anchor_length}")
    if(NOT count EQUAL 1)
        execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
            OUTPUT_VARIABLE revision OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
        if(NOT revision)
            set(revision "unknown")
        endif()
        message(FATAL_ERROR "[clap-plugin-handle] target: ${file}; expected '${anchor}' once, found ${count}; UAPMD revision: ${revision}; instructions: ${handle_dir}/README.md")
    endif()
endfunction()
handle_require_once("${handle_clap_header}" "class PluginInstanceCLAP : public PluginInstance {")
handle_require_once("${handle_clap_header}" "std::unique_ptr<CLAPPluginProxy> plugin;")
handle_require_once("${handle_upstream_instance}" "class RemidyAudioPluginInstance : public AudioPluginInstanceAPI {")
handle_require_once("${handle_upstream_instance}" "remidy::PluginInstance* rawInstance() const")

configure_file("${handle_dir}/clap.cpp.in" "${handle_output}/clap.cpp" @ONLY)
configure_file("${handle_dir}/instance.cpp.in" "${handle_output}/instance.cpp" @ONLY)
target_sources(remidy PRIVATE "${handle_output}/clap.cpp")
target_sources(uapmd-plugin-hosting PRIVATE "${handle_output}/instance.cpp")
