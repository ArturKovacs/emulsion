## About

Refer to the [website](https://arturkovacs.github.io/emulsion-website/) for an overview.

Emulsion is targeting Windows, Mac, and Linux although it is currently only being tested on Linux and Windows.

To build the software, obtain the latest stable release of [Rust](https://www.rust-lang.org/) and after navigating to the source directory simply run the command `cargo build --release` using your preferred command line interface.

Contribution is welcome. Feel free to post feature requests, bug reports, and make pull requests.

## Reporting bugs

If Emulsion closed unexpectedly please locate a "panic.txt" file which should be in the same folder as the "emulsion" executable. When posting a bug report please paste the contents of this file into GitHub. If its too long just paste the last panic entry which begins with
```
--
error message here
--
```
If there's no "panic.txt" file describe the scenario in which emulsion closed, and steps to reproduce if you deem that useful.
