## About

Refer to the [website](https://arturkovacs.github.io/emulsion-website/) for an overview.

Emulsion is targeting Windows, Mac, and Linux although it is currently only being tested on Linux and Windows.

To build the software, obtain the latest stable release of [Rust](https://www.rust-lang.org/) and after navigating to the source directory simply run the command `cargo build --release` using your preferred command line interface.

Contribution is welcome. Feel free to post feature requests, bug reports, and make pull requests.

## Custom key-bindings

To change the default key-bindings, locate the `cfg.toml` file first. For this file to be created run and then close emulsion at least once with the current user. Depending on your OS you can find it under

- Windows: `%appdata%\emulsion\config`
- MacOS: `$HOME/Library/Preferences/emulsion`
- Linux: `$XDG_CONFIG_HOME/emulsion` or `$HOME/.config/emulsion`

This file may contain a `[bindings]` section. If there is no such section, the defaults are used. If we were to add all the default bindings to this file it would look somethings like the following

```toml
dark = false
win_w = 835
win_h = 759
win_x = 109
win_y = 70

[bindings]
anim_play = ["alt+a", "alt+v"]
img_del = ["delete"]
img_fit = ["f"]
img_next = ["d", "right"]
img_orig = ["q"]
img_prev = ["a", "left"]
pan = ["space"]
present_play = ["p"]
present_play_rnd = ["alt+p"]
```

Note that all items in this section are optional so it's fully valid to only specify one of the actions. In this case all the rest will use the default bindings. For example

```toml
[bindings]
img_next = ["space", "right"]
pan = []
```

For more information and available inputs please refer to the [Bindings.md](Bindings.md) file.

## Reporting bugs

If Emulsion closed unexpectedly please locate the "panic.txt" file. This file has a different location depending on the target platform.

- Windows: `%localappdata%\emulsion\data`
- MacOS: `$HOME/Library/Application Support/emulsion`
- Linux: `$XDG_DATA_HOME/emulsion` or `$HOME/.local/share/emulsion`

When posting a bug report please upload the contents of this file to GitHub. If you deem it too large just paste the last panic entry between the rows of equal signs. If there's no "panic.txt" file describe the scenario in which emulsion closed, and steps to reproduce if you believe that could help.
