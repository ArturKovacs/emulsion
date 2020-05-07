
## General

Custom commands can be specified in the `cfg.toml` file. Read about locating or creating the file [here](README.md#custom-configuration).

This file may contain any number of `[[commands]]` sections. The default is no commands. To add a shortcut for opening the current image with Gimp on Windows one may can add the following to the `cfg.toml` file.

```toml
# Note the double brackets!
[[commands]]
input = ["alt+t", "u"]
program = "cmd"
# Note that the Gimp exe path is between single quotation marks (')
args = ["/C", "start", "", 'C:\Program Files\GIMP 2\bin\gimp-2.10.exe', "${img}"]
```

## Details

A very simple command might look like the one below.

```toml
# Note the double brackets!
[[commands]]
input = ["alt+k"]
program = "git"
```

With the above added to the `cfg.toml` file, whenever the `alt+k` key-combination is pressed, Emulsion executes `git` which prints the default git cli help message to Emulsion's standard output. As you can see `input` is an array, meaning that a single command can be bound to any number of different inputs. See the [Bindings.md](Bindings.md) for more on specifying inputs.

Any command is only executed when there's an image open.

It's important that Emulsion doesn't execute these commands in a particular shell. This means that many programs which are available from your preferred command line interface, are not available to Emulsion. With that said it is possible to execute shell commands if we specify the shell as the program itself. For example the following will print "Hello World" to the "hello.txt" file when executed from Windows.

```toml
[[commands]]
input = ["alt+k"]
program = "cmd"
args = ["/C", "echo Hello World > hello.txt"]
```

As it was previously stated, any number of `[[commands]]` can be specified.

```toml
[[commands]]
input = ["alt+k"]
program = "git"

[[commands]]
input = ["alt+l"]
program = "git"
args = ["status"]
```

There are two more parameters for each command.
- `args`: an array of arguments passed on to the program
- `envs`: an array of environment variable definitions

Within the `args`, one may use `${img}` and `${folder}` for the currently open image file path and its parent folder path respectively. Note that these are substituted with a simple find and replace so there's no need to escape dollar signs ($) and they have to be typed in the exact format specified here.

The following example specifies a single environment variable and invokes cmd with three command line arguments.

```toml
[[commands]]
input = ["alt+t", "u"]
program = "cmd"
args = ["/C", "echo", "%TEST_VAR% ${img}"]
envs = [{name = "TEST_VAR", value = "Wohoo :D"}]
```

This might for example print: `Wohoo :D \\?\D:\MyImages\mountain.jpg`.
