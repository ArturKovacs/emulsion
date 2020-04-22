## About

Refer to the [website](https://arturkovacs.github.io/emulsion-website/) for an overview.

Emulsion is targeting Windows, Mac, and Linux although it is currently only being tested on Linux and Windows.

To build the software, obtain the latest stable release of [Rust](https://www.rust-lang.org/) and after navigating to the source directory simply run the command `cargo build --release` using your preferred command line interface.

Contribution is welcome. Feel free to post feature requests, bug reports, and make pull requests.

## Reporting bugs

If Emulsion closed unexpectedly please locate the "panic.txt" file. This file has a different location depending on the target platform.

- Windows: `%localappdata%\emulsion\data`
- MacOS: `$HOME/Library/Application Support/emulsion`
- Linux: `$XDG_DATA_HOME/emulsion` or `$HOME/.local/share/emulsion`

When posting a bug report please upload the contents of this file to GitHub. If you deem it too large just paste the last panic entry between the rows of equal signs. If there's no "panic.txt" file describe the scenario in which emulsion closed, and steps to reproduce if you believe that could help.
