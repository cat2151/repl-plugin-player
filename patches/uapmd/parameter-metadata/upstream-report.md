# Draft: preserve actual CLAP parameter readonly metadata

Not sent. Verified against a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe.

`UapmdJSRuntime.cpp` reports every parameter writable. The CLAP parameter
constructor hardcodes automatable and assigns the automatable flag to readable;
neither conveys CLAP readonly. A host UI can consequently enable controls that
the plugin says cannot be changed. Please expose actual readonly/stepped flags
through the hosting metadata and JS API, with unknown formats distinguishable.

Our local supplement observes CLAP get_info on the main thread and keeps controls
disabled when ownership, index or stable ID cannot be verified. Refresh/lifetime
epochs prevent stale controls from targeting a replacement parameter. No upstream
checkout is changed. Tests include readonly/unknown/refresh/lifetime fixtures and
silent real Surge read/write with original values restored.
