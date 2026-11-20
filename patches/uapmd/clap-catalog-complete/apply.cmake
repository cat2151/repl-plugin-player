set(clap_catalog_dir "${CMAKE_CURRENT_LIST_DIR}")
set(clap_catalog_upstream "${UAPMD_DIR}/source/uapmd-plugin-hosting/src/scanner/PluginScanTool.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${clap_catalog_upstream}")
# The skip this patch works around. When upstream changes it, the patch may be
# unnecessary or wrong; README.md says how to decide.
set(clap_catalog_anchor "        if (!forceRescan && hasCachedSlowEntries)\n            continue;\n")
file(READ "${clap_catalog_upstream}" clap_catalog_text)
string(REPLACE "${clap_catalog_anchor}" "" clap_catalog_without "${clap_catalog_text}")
string(LENGTH "${clap_catalog_text}" clap_catalog_with_length)
string(LENGTH "${clap_catalog_without}" clap_catalog_without_length)
string(LENGTH "${clap_catalog_anchor}" clap_catalog_anchor_length)
math(EXPR clap_catalog_count "(${clap_catalog_with_length} - ${clap_catalog_without_length}) / ${clap_catalog_anchor_length}")
if(NOT clap_catalog_count EQUAL 1)
    execute_process(COMMAND git -C "${UAPMD_DIR}" rev-parse HEAD
        OUTPUT_VARIABLE clap_catalog_revision OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
    message(FATAL_ERROR "[clap-catalog-complete] target: ${clap_catalog_upstream}; expected the cached slow-scan skip once, found ${clap_catalog_count}; UAPMD revision: ${clap_catalog_revision}; instructions: ${clap_catalog_dir}/README.md")
endif()
target_sources(uapmd-plugin-hosting PRIVATE "${clap_catalog_dir}/clap_catalog.cpp")
