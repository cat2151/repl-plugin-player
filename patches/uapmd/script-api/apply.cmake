# Add one host callback to an app-owned copy. Never modify the UAPMD checkout.
execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
    OUTPUT_VARIABLE script_api_revision OUTPUT_STRIP_TRAILING_WHITESPACE)
set(script_api_input "${UAPMD_DIR}/source/tools/uapmd-app-model/src/UapmdJSRuntime.cpp")
set(script_api_output "${REPL_PROJECT_ROOT}/target/shim/script-api/UapmdJSRuntime.cpp")
function(script_api_failure reason)
    message(FATAL_ERROR "script-api: ${script_api_input}: ${reason}; upstream ${script_api_revision}; see ${CMAKE_CURRENT_FUNCTION_LIST_DIR}/README.md")
endfunction()
if(NOT EXISTS "${script_api_input}")
    script_api_failure("runtime source missing")
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${script_api_input}")
file(READ "${script_api_input}" script_api_source)
set(script_api_anchor "void UapmdJSRuntime::registerConsoleFunctions()\n{")
string(REGEX MATCHALL "void UapmdJSRuntime::registerConsoleFunctions\\(\\)" script_api_matches "${script_api_source}")
list(LENGTH script_api_matches script_api_match_count)
string(FIND "${script_api_source}" "${script_api_anchor}" script_api_position)
if(NOT script_api_match_count EQUAL 1 OR script_api_position EQUAL -1)
    script_api_failure("expected unique console registration anchor missing")
endif()
string(REPLACE "${script_api_anchor}" "${script_api_anchor}
    impl_->context.registerFunction(\"__repl_host_call\", [](choc::javascript::ArgumentList args) -> choc::value::Value {
        auto request = args.get<std::string>(0, \"\");
        return choc::value::createString(std::string(repl_script_api(request.c_str())));
    });" script_api_source "${script_api_source}")
string(PREPEND script_api_source "extern \"C\" const char* repl_script_api(const char*);\n")
# loadPluginStateSync waits for a completion queued to this same thread; drain the queue
# while waiting, as __remidy_project_load does.
set(script_api_load_state "auto result = uapmd_app::AppModel::instance().loadPluginStateSync (instanceId, filepath);")
string(REGEX MATCHALL "loadPluginStateSync \\(instanceId, filepath\\)" script_api_load_matches "${script_api_source}")
list(LENGTH script_api_load_matches script_api_load_count)
string(FIND "${script_api_source}" "${script_api_load_state}" script_api_load_position)
if(NOT script_api_load_count EQUAL 1 OR script_api_load_position EQUAL -1)
    script_api_failure("expected unique loadPluginStateSync call in __remidy_instance_load_state missing")
endif()
string(REPLACE "${script_api_load_state}" "auto loadPromise = std::make_shared<std::promise<uapmd_app::AppModel::PluginStateResult>>();
        auto loadFuture = loadPromise->get_future();
        uapmd_app::AppModel::instance().loadPluginState (instanceId, filepath,
            [loadPromise] (uapmd_app::AppModel::PluginStateResult r) { loadPromise->set_value (std::move (r)); });
        while (loadFuture.wait_for (std::chrono::seconds (0)) != std::future_status::ready)
        {
            if (remidy::EventLoop::runningOnMainThread())
                remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for (std::chrono::milliseconds (1));
        }
        auto result = loadFuture.get();" script_api_source "${script_api_source}")
get_target_property(script_api_sources uapmd-app-model SOURCES)
list(FIND script_api_sources "src/UapmdJSRuntime.cpp" script_api_source_index)
if(script_api_source_index EQUAL -1)
    script_api_failure("runtime target source missing")
endif()
file(MAKE_DIRECTORY "${REPL_PROJECT_ROOT}/target/shim/script-api")
file(WRITE "${script_api_output}" "${script_api_source}")
list(REMOVE_ITEM script_api_sources "src/UapmdJSRuntime.cpp")
list(APPEND script_api_sources "${script_api_output}")
set_property(TARGET uapmd-app-model PROPERTY SOURCES "${script_api_sources}")
# The copy includes runtime-private headers relative to the original source.
target_include_directories(uapmd-app-model PRIVATE "${UAPMD_DIR}/source/tools/uapmd-app-model/src")
