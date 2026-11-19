// The reload snippet runs without error, and the catalog it reads again still answers.
// Test-owned script: the snippet's contents are never expectations.

// %% held catalog
{
    const before = repl.patchCatalog({role:'chord', kind:'pad'});
    e2e.assert(Array.isArray(before), 'Catalog is not an array');
    before.length
}

// %% reload snippet
{
    const source = __replCall('snippet.source', {name:'builtin/reload-patch-catalog.js'});
    for (const cell of source.split(/^\/\/ %%.*$/m).filter(c => c.trim())) (0, eval)(cell);
    const after = repl.patchCatalog({role:'chord', kind:'pad'});
    e2e.assert(Array.isArray(after), 'Catalog is not an array after the reload');
    after.length
}
