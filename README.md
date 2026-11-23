# repl-plugin-player

Play audio plugins with a [REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop)-like UI. It uses [UAPMD](https://github.com/atsushieno/uapmd) and is written in Rust.

It has peculiar features and is almost exclusively for personal use. Specifications may change tomorrow, potentially rendering existing files unreadable.

## Download

[Download Windows version](https://github.com/cat2151/repl-plugin-player/releases/download/v0.1.0/repl-plugin-player-0.1.0-windows-x64.zip)

This is an experimental project, so it might occasionally return 404 errors.

## Past Challenges and What This App Solves

### Playing Audio Plugins with Scripts

- DAWs are excellent for high-quality music composition and arrangement using audio plugins!
  - However, to play audio plugins, you need to learn "how to use a DAW."
    - This is particularly true for mainstream DAWs, which have a high learning curve.
- Live coding is amazing for making music with programs!
  - Browser-based playgrounds also make it easy to get started.
  - Nevertheless, you still need to learn a specific syntax, which presents a slight hurdle until you get used to it.
  - Furthermore, I haven't found anything that natively supports large VST3/CLAP audio plugins by default, allowing them to be played immediately after launching with a single binary.
- Back in the era of Family BASIC and MSX, you could play "do-re-mi" with commands like `PLAY "CDE"`.
  - That's intuitive!
  - And the `CDE` part could be generated on the fly with a program.
- I wanted to create a playful experience where, after launching the app, you could type `play("cde")` to play VST3/CLAP audio plugins, and program the performance content itself to play it instantly (without time-consuming builds).
- In modern times, if you have a desired UX, you can achieve it with live coding.
- So, I decided to do just that.

## Concept, What it Aims For

- Toy
  - Play with a clear understanding of its limitations.
- For personal use
  - Frequent breaking changes. Specifications may change tomorrow, potentially rendering existing files unreadable.
- Scripting
  - Just by writing a script,
    - You can use the plugin hosting library API. No time-consuming builds required.
    - You can programmatically control VST3/CLAP audio plugins.
- DLL
  - You can add functionality by building your own DLLs (probably, under development).
- Rich sounds
  - Can play VST3/CLAP audio plugins.
- Ease of use
  - Instant sound.
    - Just choose snippets. Easy!
  - Guidance.
    - If you're in an environment without audio plugins, it guides you to easily install Surge XT. Easy!
  - Lightweight.
    - Just download a standalone .exe from GitHub releases and run it. Easy!
- Visualization
  - Minimal, simple visualizer.

## What it Does Not Aim For

- Music programming language
  - [SuperCollider](https://supercollider.github.io/downloads) / [SonicPi](https://sonic-pi.net/code.html) , [TidalCycles](https://tidalcycles.org/) / [Strudel](https://strudel.cc/#JDogcygiW2JkIDxoaCBvaD5dKjIiKS5iYW5rKCJ0cjkwOSIpLmRlYyguNCk%3D) , [Glicol](https://glicol.org/demo)
- Maximum performance pursuit.
- Infinitely rich built-in snippets.
- Rapid bug fixes.
- Responding to user requests.

## Supplementary Notes
- Supplementary Image/Concept
  - Play with rich timbres programmatically.
    - Richer than various browser-based timbres (as it can fully play native VST3/CLAP sounds).
    - More programmable than MML.
    - With more script-like syntax and coding content than music programming languages.
    - With simpler environment setup than music programming languages that use VSCode (just download and run an exe from GitHub).
    - Perhaps faster iteration than building native applications each time (depending on the use case).
    - More programmable than most DAWs. More extensible.
    - Faster startup than DAWs.
  - Usage Scenarios
    - With a browser, it plays regardless of the device.
    - With MML, you can quickly build and enjoy playing music (scores) with less descriptive effort.
    - With music programming languages, you can express at the waveform level and write diverse content more quickly.
    - With music programming languages that use VSCode, you can get results faster with more efficient keystroking.
    - DAWs and native applications might be more feature-rich, higher-performance, and stable.
    - Among DAWs, REAPER might be even more programmable, extensible, feature-rich, high-performance, and stable.
- No compatibility
  - Given the powerful UAPMD DAW engine and the scripting/DLL mechanism, various things might be possible, such as building a DAW or a music programming language layer on top of it.
    - However, this project undergoes frequent breaking changes.
      - Therefore, in reality, there's a high probability that "even if you build something elaborate, it might become unplayable, cease to function, or crash with an error the next day."
        - The migration cost would be significant (even though AI might offer possibilities for cost reduction).
          - So, instead of elaborate builds, let's just play on a small scale.

## For Developers

Build

```
python scripts/build.py
```

## License

MIT