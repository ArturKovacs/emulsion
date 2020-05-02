## About

Refer to the [website](https://arturkovacs.github.io/emulsion-website/) for an overview.

Emulsion is targeting Windows, Mac, and Linux although it is currently only being tested on Linux and Windows. A note for Linux users: Wayland support is limited, so for example expect high CPU usage and the title text not being shown. However X is fully supported.

To build the software, obtain the latest stable release of [Rust](https://www.rust-lang.org/) and after navigating to the source directory simply run the command `cargo build --release` using your preferred command line interface.

Contribution is welcome. Feel free to post feature requests, bug reports, and make pull requests.

## Custom configuration

The `cfg.toml` file allows for some modifications in the behavour of emulsion. 

Depending on the platform this file can be found or created at the following location.

- Windows: `%appdata%\emulsion\config\cfg.toml`
- MacOS: `$HOME/Library/Preferences/emulsion/cfg.toml`
- Linux: `$XDG_CONFIG_HOME/emulsion/cfg.toml` or `$HOME/.config/emulsion/cfg.toml`

The contenst of the `cfg.toml` file may for example be the following:

```toml
[bindings]
img_next = ["k"]
img_prev = ["j"]

[updates]
check_updates = true   # set to false to disable checking for updates
```

Currently the only valid sections are: `[bindings]` and `[updates]`. All sections in this file are optional, meaning
that if for example only `[updates]` is specified then the default key-bindings will be used.

The `[updates]` section can contain only one field, namely `check_updates` which may be set to either `true` or `false`.
Emulsion fetches the latest version number and provides a notification only if `check_updates` is set to `true`.
The default value is `true`. (Note that this field has no effect when emulsion is compiled without networking.)

This file may contain a `[bindings]` section which allows defining custom key-bindings.
For more on that please refer to the [Bindings.md](Bindings.md) file.

## Notes on Networking

When installing Emulsion through a perpared package like the Windows installer, Emulsion will have networking enabled and by default
will check for updates. However the default feature-set for emulsion does not include networking. This means that Emulsion will
not have networking dependent capabilities when invoking
```
cargo install emulsion
```

To enable such features with this method, run
```
cargo install emulsion --features=networking
```

## Reporting bugs

If Emulsion closed unexpectedly please locate the `"panic.txt"` file. This file has a different location depending on the target platform.

- Windows: `%localappdata%\emulsion\data`
- MacOS: `$HOME/Library/Application Support/emulsion`
- Linux: `$XDG_DATA_HOME/emulsion` or `$HOME/.local/share/emulsion`

When posting a bug report please upload the contents of this file to GitHub. If you deem it too large just paste the last panic entry between the rows of equal signs. If there's no `"panic.txt"` file, describe the scenario in which you experienced the faulty behaviour, and steps to reproduce it if you believe that could help.
