# audiostream.rs

A library for handling audio streams for the [Rust] programming language,
inspired by [gstreamer]. Compared to gstreamer, the goals are improved
robustness and simplified usage for client applications.

[Rust]: http://www.rust-lang.org/
[gstreamer]: http://gstreamer.freedesktop.org/

## Usage

Example applications can be found in the `examples` directory within this
repository. Library documentation can be generated with `rustdoc`.

If using [cargo] to build your application, the following snippet will add
`audiostream` as a dependency in your `Cargo.toml`:

    [dependencies.audiostream]
    git = "https://github.com/tari/audiostream.rs"

To build manually, the canonical version of the library exists at
http://bitbucket.org/tari/audiostream.rs It depends on the [rust-ao] bindings
to [libao] for audio output.

[cargo]: http://crates.io/
[rust-ao]: https://bitbucket.org/tari/rust-ao
[libao]: https://www.xiph.org/ao/

## License

This software is provided under the terms of the ISC license. You are free
to use it in any way, provided the included copyright notice is preserved.
See the included `COPYING` file for full license text.
