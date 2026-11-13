# repl-plugin-player

Play audio plugins with a [REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop)-like UI. It leverages [UAPMD](https://github.com/atsushieno/uapmd) and is written in Rust.

This project has some quirks and is almost exclusively for my personal use. Specifications may change tomorrow, potentially rendering existing files unreadable.

## Download

[Download Windows version](https://github.com/cat2151/repl-plugin-player/releases/download/v0.1.0/repl-plugin-player-0.1.0-windows-x64.zip)

This is an experimental project, so the link might occasionally return a 404 error.

## Past Challenges and What This App Solves

### Playing Audio Plugins with Scripts

- DAWs are wonderful for high-quality composition and arrangement using audio plugins!
  - However, to play audio plugins, you need to learn "how to use a DAW."
    - This is especially true for mainstream DAWs, which can be a high hurdle for new users.
- Live coding is amazing for creating music programmatically!
  - Browser-based playgrounds are available, making it easy to get started.
  - Nevertheless, you still need to learn a dedicated syntax, which presents a bit of a hurdle until you're familiar with it.
  - Furthermore, I couldn't find anything that natively supports large-scale VST3/CLAP audio plugins by default and allows you to play them immediately after launching a single binary.
- Back in the days of Family Basic and MSX, you could play musical notes like "Do-Re-Mi" with commands such as `PLAY "CDE"`.
  - That's intuitive!
  - And the `CDE` part could even be generated on the fly programmatically.
- I wanted to explore a playful experience where, upon launching an app, I could type `play("cde")` to make a VST3/CLAP audio plugin produce sound, and program the musical content itself, playing it instantly (without time-consuming builds).
- Today, if you have a desired UX, you can achieve it through coding.
- So, I decided to do just that.

## Concepts and Goals

-   **A Toy**
    -   Play with it without expecting production readiness.
-   **For Personal Use**
    -   I will frequently make breaking changes. Specifications may change tomorrow, potentially rendering existing files unreadable.
-   **Scripting**
    -   Just by writing scripts:
        -   You can utilize plugin hosting library APIs. No time-consuming builds required.
        -   You can programmatically control VST3/CLAP audio plugins.
-   **DLL**
    -   You can add functionality by building your own DLLs (probably, under development).
-   **Rich Timbres**
    -   Capable of playing VST3/CLAP audio plugins.
-   **Ease of Use**
    -   **Instant Sound**
        -   Just select snippets. Easy!
    -   **Guidance**
        -   If there's no audio plugin environment, it guides users to easily install Surge XT. Easy!
    -   **Lightweight**
        -   Just download a standalone executable from GitHub releases and run it. Easy!
-   **Visualization**
    -   A minimal, simple visualizer.

## What it is NOT

-   **A Music Programming Language**
    -   [SuperCollider](https://supercollider.github.io/downloads) / [SonicPi](https://sonic-pi.net/code.html) , [TidalCycles](https://tidalcycles.org/) / [Strudel](https://strudel.cc/#JDogcygiW2JkIDxoaCBvaD5dKjIiKS5iYW5rKCJ0cjkwOSIpLmRlYyguNCk%3D) , [Glicol](https://glicol.org/demo)
-   **Optimized for Extreme Performance**
-   **Endlessly Rich Built-in Snippets**
-   **Rapid Bug Fixes**
-   **Responsive to User Requests**

## Additional Notes

-   **Clarifying the Vision**
    -   **Playing with rich timbres programmatically**
        -   Richer than various browser sounds (because it fully utilizes native VST3/CLAP timbres).
        -   More programmable than MML.
        -   Uses a more script-like syntax and coding content than music programming languages.
        -   Simpler environment setup than music programming languages that use VSCode (just download and run an exe from GitHub).
        -   May allow for faster iteration than building native applications repeatedly (depending on the use case).
    -   **Distinctions in Use Cases**
        -   Browsers play sound on any device.
        -   MML allows for quicker construction and enjoyment of music (scores) with less code.
        -   Music programming languages offer expression down to waveform units and enable quicker, more diverse descriptions.
        -   Music programming languages using VSCode can yield faster results with more efficient keystrokes.
        -   Native applications can produce richer (more computationally intensive) sounds with larger orchestrations.
-   **Lack of Compatibility**
    -   Given the powerful UAPMD DAW engine and its scripting/DLL mechanisms, it might be possible to build various layers on top, such as a DAW or a music programming language.
    -   **However, this project undergoes frequent breaking changes.**
        -   Therefore, in reality, there's a high probability that "even if you build something on top, it might become unplayable, stop working, or crash due to errors the next day."
        -   The cost of migration would likely be substantial (even if AI could potentially mitigate costs).
        -   Thus, instead of building complex systems, let's keep it small-scale and playful.

## License

MIT