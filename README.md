# repl-plugin-player

Play audio plugins with a [REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop)-like UI. It uses [UAPMD](https://github.com/atsushieno/uapmd). Written in Rust.

It has some quirks. It's primarily for personal use. Specifications may change tomorrow, potentially making existing files unreadable.

## Download

[Download Windows Version](https://github.com/cat2151/repl-plugin-player/releases/download/v0.1.0/repl-plugin-player-0.1.0-windows-x64.zip)

This is an experimental project, so it may occasionally return a 404 error.

## Past Challenges and What This App Solves

### Playing Audio Plugins with Scripts

- DAWs are great for high-quality composition and arrangement with audio plugins!
  - However, to play audio plugins, you need to learn "how to use a DAW."
    - This is especially true for mainstream DAWs, making the initial learning curve steep.
- Live coding is great for making music with programs!
  - There are browser playgrounds available, making it easy to get started.
  - Nevertheless, you still need to learn a dedicated syntax, which presents a bit of a hurdle until you get used to it.
  - Furthermore, I haven't found anything that natively supports large VST3/CLAP audio plugins by default and can play them immediately after launching with a single binary.
- Back in the days of Family BASIC and MSX, you could play musical notes (do-re-mi) with commands like `PLAY "CDE"`.
  - That's intuitive, isn't it?
  - And the `CDE` part could even be generated on the fly by a program.
- I wanted to play around with an application where, after launching it, you could enter `play("cde")` to make VST3/CLAP audio plugins play, and program the performance content itself to play it instantly (without time-consuming builds).
- In modern times, if you have a desired UX, you can achieve it with live coding.
- That's what I decided to do.

## Concept, What It Aims For

- A Toy
  - Meant for playful use
- For Personal Use
  - I will frequently make breaking changes. Specifications may change tomorrow, potentially making existing files unreadable.
- Scripting
  - Simply by writing scripts, you can:
    - Utilize plugin hosting library APIs. No time-consuming builds required.
      - Programmatically control VST3/CLAP audio plugins.
- DLL
  - If you build your own DLLs, you can add features (probably. Under development).
- Rich Tones
  - Can play VST3/CLAP audio plugins.
- Ease of Use
  - Instant sound playback
    - Just select snippets. Easy!
  - Guidance
    - If you don't have an audio plugin environment, it guides you to easily install Surge XT. Easy!
  - Lightweight
    - Just download and run the standalone executable from GitHub releases. Easy!
- Visualization
  - A minimal, simple visualiser.

## What It Does Not Aim For

- A Music Programming Language
  - [SuperCollider](https://supercollider.github.io/downloads) / [SonicPi](https://sonic-pi.net/code.html) , [TidalCycles](https://tidalcycles.org/) / [Strudel](https://strudel.cc/#JDogcygiW2JkIDxoaCBvaD5dKjIiKS5iYW5rKCJ0cjkwOSIpLmRlYyguNCk%3D) , [Glicol](https://glicol.org/demo)
- Extreme performance optimization
- Infinitely rich built-in snippets
- Rapid bug fixes
- Responding to user requests

## Supplementary Notes
- Clarifying the Vision
  - Play with rich tones programmatically
    - Richer than various browser-based tones (because it can fully play native VST3/CLAP tones)
    - More programmable than MML
    - With a more script-like syntax and coding content than music programming languages
    - With simpler environment setup than music programming languages that use VSCode (just download and run the executable from GitHub)
    - May allow for faster iteration than building native applications repeatedly (depending on the use case)
    - More programmable and extensible than most DAWs
    - Faster to launch than DAWs
- Distinguishing Use Cases
  - Browsers offer playback regardless of device.
  - MML allows quicker composition and playback of music (scores) with less code.
  - Music programming languages enable waveform-level expression and faster, more diverse coding.
  - Music programming languages using VSCode offer more efficient typing and quicker results.
  - DAWs and native applications might be more feature-rich, high-performance, and stable.
  - Among DAWs, REAPER might be even more programmable, extensible, feature-rich, high-performance, and stable.
- Lack of Compatibility
  - Given the powerful DAW engine UAPMD and its scripting/DLL mechanisms, it would be possible to layer DAWs or music programming languages on top of it, among other things.
    - However, this project will frequently introduce breaking changes.
      - Therefore, in reality, there's a high probability that "even if you build on it, it might become unplayable, non-functional, or crash with errors the next day."
        - The migration costs would likely be significant (even if AI could potentially mitigate them).
          - So, instead of extensive development, let's keep it small-scale and playful.

## License

MIT