## About

Refer to the [website](https://arturkovacs.github.io/emulsion-website/) for an overview.

Emulsion is targeting Windows, Mac, and Linux although it is currently only being tested on Linux and Windows. A note for Linux users: Wayland support is limited, so for example expect high CPU usage and the title text not being shown. However X is fully supported.

Planned releases are represented with Milestones (under Issues). I try making a new release every other month or so, but don't take the deadline too seriously. If there's a feature or bugfix that's particularly interesting to you, please indicate this at the issue - a reaction like a thumbs up might just be enough but sometimes it's better to leave a comment because that's what I get a notification about.

Contribution is welcome. Feel free to post feature requests, bug reports, and make pull requests.

## Building and Installing

It is recommended to use the officially provided installer found on the website and at the GitHub releases page. Although there can be a few resons why someone wants to build from source. For this, it's required to have the latest stable release of [Rust](https://www.rust-lang.org/) installed; proceed when that's done.

In many cases it's a good start to try running `cargo install emulsion`. If that build fails or if emulsion panics on startup, look into the `nix-example/emulsion/default.nix` file and locate `rpathLibs` which lists the libraries that emulsion depends on. Install the dev version of those libraries then try running the build/install again. For example on Ubuntu one can install `libXi` by running

```
sudo apt install libXi-dev
```

For the [Nix Package Manager](https://nixos.wiki/wiki/Nix) users: The Nix expressions found within `nix-example` is in theory able to build a working executable from *a* state of the emulsion source code. There is no guarantee that the built executable will be identical to any released version of emulsion. The Nix expression is provided to find the dependencies and for those who like tinkering with Nix but otherwise I advise against using it.

### Notes about Cargo Features

All packages on the website come with avif support, however it is not a default feature as the dependecies are not trivial to set up. If you are bulding from source (eg using `cargo install`) and would like emulsion to open avif files, I recommend taking a look at the [release workflow](.github/workflows/release-packages.yml) for steps to install the avif development dependencies.

When installing Emulsion through the Windows installer, Emulsion will have networking enabled and will by default check for updates. However none of the other versions have networking and neither does the default feature-set. This also means that Emulsion will not have networking dependent capabilities when invoking
```
cargo install emulsion
```

To enable such features when installing with cargo, run
```
cargo install emulsion --features=networking
```

## Reporting Bugs

If Emulsion closed unexpectedly please locate the `"panic.txt"` file. This file has a different location depending on the target platform.

- Windows: `%localappdata%\emulsion\data`
- MacOS: `$HOME/Library/Application Support/emulsion`
- Linux: `$XDG_DATA_HOME/emulsion` or `$HOME/.local/share/emulsion`

When posting a bug report please upload the contents of this file to GitHub. If you deem it too large just paste the last panic entry between the rows of equal signs. If there's no `"panic.txt"` file, describe the scenario in which you experienced the faulty behaviour, and steps to reproduce it if you believe that could help.
