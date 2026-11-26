#pragma once
#include <string>
extern "C" const char* repl_localized(const char* key);

// Substitute named fields in one pass, never inside external diagnostics.
inline std::string repl_format1(const char* key, const char* name, const std::string& value) {
    const std::string text = repl_localized(key);
    const std::string token = std::string("{$") + name + "}";
    std::string result;
    size_t from = 0;
    while (true) {
        const auto at = text.find(token, from);
        if (at == std::string::npos) { result += text.substr(from); break; }
        result += text.substr(from, at - from);
        result += value;
        from = at + token.size();
    }
    return result;
}
