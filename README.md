# repl-plugin-player

Play audio plugins with a [REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop)-like UI. Utilizes [UAPMD](https://github.com/atsushieno/uapmd). Written in Rust.

The features are somewhat quirky. It's almost exclusively for personal use. Specifications may change tomorrow, potentially rendering previous files unreadable.

## Download

[Download Windows version](https://github.com/cat2151/repl-plugin-player/releases/download/v0.1.0/repl-plugin-player-0.1.0-windows-x64.zip)

Since this is an experimental project, it might occasionally return 404 errors.

## Past Challenges and What This Application Solves

### Playing Audio Plugins with Scripts

- DAWs are great for high-quality composition and arrangement with audio plugins!
  - However, to play audio plugins, you need to learn "how to use a DAW."
    - This is especially true for mainstream DAWs, where the learning curve can be steep until you get used to them.
- Live coding is great for creating music with programs!
  - Browser-based playgrounds also make it easy to get started.
  - Nevertheless, you need to learn a dedicated syntax, which presents a slight hurdle until you're familiar with it.
  - Furthermore, I haven't found anything that natively supports large VST3/CLAP audio plugins by default and can play them immediately after launching with a single binary.
- Back in the era of Family Basic and MSX, you could play "do-re-mi" with notations like `PLAY "CDE"`.
  - That's intuitive!
  - And the `CDE` part could even be generated on the fly with a program.
- I wanted to experience the fun of launching an application, typing `play("cde")`, and hearing VST3/CLAP audio plugins, or programming the performance itself and playing it instantly (without time-consuming builds).
- Today, if you have a desired UX, you can achieve it through "vibecoding."
- So, that's what I decided to do.

## Concept and Goals

- Toy
  - Designed for lighthearted fun.
- For personal use
  - Frequent breaking changes will occur. Specifications may change tomorrow, potentially rendering previous files unreadable.
- Scripting
  - Simply by writing scripts,
    - You can utilize plugin hosting library APIs. No time-consuming builds required.
      - You can programmatically control VST3/CLAP audio plugins.
- DLL
  - You can add features by building your own DLLs (probably, currently under development).
- Rich Tones
  - Can play VST3/CLAP audio plugins.
- Ease of Use
  - Instant Sound
    - Just select snippets. Easy!
  - Guidance
    - If you're in an environment without audio plugins, it guides you to easily install Surge XT. Easy!
  - Lightweight
    - Just download the standalone executable from GitHub releases and run it. Easy!
- Visualization
  - Minimal, simple visualizer.

## What It Is Not

- Music Programming Language
  - [SuperCollider](https://supercollider.github.io/downloads) / [SonicPi](https://sonic-pi.net/code.html) , [TidalCycles](https://tidalcycles.org/) / [Strudel](https://strudel.cc/#JDogcygiW2JkIDxoaCBvaD5dKjIiKS5iYW5rKCJ0cjkwOSIpLmRlYy(.4) , [Glicol](https://glicol.org/demo)
- Extreme Performance Optimization
- Infinitely Rich Built-in Snippets
- Rapid Bug Fixes
- Responding to User Requests

## Supplementary Notes
- Clarifying the Concept
  - Play with rich tones programmatically
    - Richer than various browser-based tones (because it can fully play native VST3/CLAP tones).
    - More programmable than MML.
    - With more script-like syntax and coding content than a music programming language.
    - With simpler environment setup than music programming languages that use VSCode (just download and run the exe from GitHub).
    - Potentially faster iteration cycles than repeatedly building native applications (depending on the use case).
    - More programmable than most DAWs. More extensible.
    - Faster startup than DAWs.
  - Distinction in Use
    - Browsers play sounds regardless of device.
    - MML allows for quicker creation and enjoyment of music (scores) with less descriptive text.
    - Music programming languages allow for waveform-level expression and faster, more diverse notation.
    - Music programming languages that use VSCode allow for more efficient key-typing and faster results.
    - DAWs and native applications might be more feature-rich, high-performance, and stable.
    - Among DAWs, REAPER might be even more programmable, extensible, feature-rich, high-performance, and stable.
- No Compatibility
  - Given the powerful UAPMD DAW engine, scripting, and DLL mechanisms, many things should be possible, such as building a DAW or music programming language layer on top of it.
    - However, this project undergoes frequent breaking changes.
      - Therefore, in reality, there's a high probability that "even if you build something elaborate, it might become unplayable, stop working, or crash with an error the next day."
        - The migration cost would be significant (even though AI might help reduce it).
          - Thus, it's advisable not to build anything extensive and instead, play around on a small scale.

## License

MIT