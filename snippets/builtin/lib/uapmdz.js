// What a uapmdz project archive needs to sound: the plugins its track graphs name,
// and the sound files (sfz and the like) their saved plugin states name.
(() => {
    // A URI keeps its query: AAP sfizz names the soundpack in `?...&via=`.
    const SOUND_FILE_END = /\.(?:sfz|sf2|sf3|dspreset|wav|flac|ogg)(?![A-Za-z0-9])(?:\?[^\x00-\x20\x7f"<>]*)?/gi;
    const PATH_CHAR = /[^\x00-\x1f\x7f"<>|*?]/;
    const PATH_LIMIT = 1024;
    // The first byte after a zlib stream's 0x78.
    const ZLIB_LEVELS = [0x01, 0x5e, 0x9c, 0xda];

    function latin1(bytes) {
        let text = '';
        for (let i = 0; i < bytes.length; i += 8192)
            text += String.fromCharCode.apply(null, bytes.slice(i, i + 8192));
        return text;
    }

    function utf8(bytes) {
        const text = latin1(bytes);
        try {
            return decodeURIComponent(escape(text));
        } catch (error) {
            return text;
        }
    }

    // Entry name -> bytes. uapmd stores the members without compression.
    function entries(bytes) {
        const u16 = at => bytes[at] | bytes[at + 1] << 8;
        const u32 = at => (u16(at) | u16(at + 2) << 16) >>> 0;
        let end = bytes.length - 22;
        while (end >= 0 && u32(end) !== 0x06054b50) end--;
        if (end < 0) throw new Error('Not a zip archive');
        const files = {};
        let at = u32(end + 16);
        for (let count = u16(end + 10); count > 0; count--) {
            if (u32(at) !== 0x02014b50) throw new Error('Broken zip central directory');
            const name = utf8(bytes.slice(at + 46, at + 46 + u16(at + 28)));
            if (u16(at + 10) !== 0) throw new Error(name + ': compressed zip entries are not supported');
            const local = u32(at + 42);
            const data = local + 30 + u16(local + 26) + u16(local + 28);
            files[name] = bytes.slice(data, data + u32(at + 20));
            at += 46 + u16(at + 28) + u16(at + 30) + u16(at + 32);
        }
        return files;
    }

    // The state as stored, and every zlib stream inside it inflated (sforzando keeps its sfz path in one).
    function soundFiles(state) {
        const texts = [latin1(state)];
        for (let i = 0; i + 1 < state.length; i++) {
            if (state[i] !== 0x78 || !ZLIB_LEVELS.includes(state[i + 1])) continue;
            try {
                texts.push(latin1(repl.zlib.decompress(state.slice(i))));
            } catch (error) {
                // Not a zlib stream, only bytes that look like its header.
            }
        }
        return texts.flatMap(paths).map(path => utf8(Array.from(path, c => c.charCodeAt(0))));
    }

    // Finds the extensions first and walks back to each path's start: one regex
    // spanning the whole path backtracks quadratically over long runs of text.
    function paths(text) {
        const found = [];
        let previousEnd = 0;
        SOUND_FILE_END.lastIndex = 0;
        for (let match; (match = SOUND_FILE_END.exec(text)) !== null;) {
            let start = match.index;
            while (start > previousEnd && match.index - start < PATH_LIMIT && PATH_CHAR.test(text[start - 1])) start--;
            previousEnd = match.index + match[0].length;
            if (start < match.index) found.push(text.slice(start, previousEnd));
        }
        return found;
    }

    // {plugins:[{format, displayName, pluginId}], files:[path]}, each listed once in archive order.
    function requirements(path) {
        const files = entries(repl.file.read(path));
        const plugins = new Map();
        const soundPaths = new Set();
        for (const name of Object.keys(files).filter(name => /^graphs\/.+\.graph\.json$/.test(name))) {
            for (const plugin of JSON.parse(utf8(files[name])).plugins || []) {
                plugins.set(plugin.format + '\n' + plugin.plugin_id,
                    {format:plugin.format, displayName:plugin.display_name, pluginId:plugin.plugin_id});
                if (plugin.state_file && files[plugin.state_file])
                    soundFiles(files[plugin.state_file]).forEach(file => soundPaths.add(file));
            }
        }
        return {plugins:Array.from(plugins.values()), files:Array.from(soundPaths)};
    }

    function describe(path) {
        const needed = requirements(path);
        const lines = [path, 'plugins:'].concat(needed.plugins.map(p => '  ' + p.format + ' ' + p.displayName));
        if (needed.files.length > 0) lines.push('files:', ...needed.files.map(file => '  ' + file));
        return lines.join('\n');
    }

    // Every .uapmdz under the directory, subdirectories included, in name order.
    function find(directory) {
        return repl.file.list(directory).flatMap(entry => {
            const path = directory + '/' + entry.name;
            if (entry.directory) return find(path);
            return /\.uapmdz$/i.test(entry.name) ? [path] : [];
        });
    }

    return {requirements, describe, find};
})();
