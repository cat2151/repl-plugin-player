// %% @text name_api_list
// @text api_list_comment
Object.keys(globalThis)
    .filter(function(name) { return !name.startsWith('__'); })
    .map(function(name) {
        var value = globalThis[name];
        if (value === null || typeof value !== 'object') return name + ': ' + typeof value;
        return name + ': ' + Object.keys(value).join(', ');
    })
    .join(' / ');
