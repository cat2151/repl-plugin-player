{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', bpm:120});
    Object.assign(globalThis, easy);
    easy.reset();
    repl.text('easy_functions', {names:Object.keys(easy).join(', ')}) + '\n' + repl.text('easy_try_play')
}
