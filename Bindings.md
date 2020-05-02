
## General

Custom key-bindings can be specified in the `cfg.toml` file. Read about locating or creating the file [here](README.md#custom-configuration).

This file may contain a `[bindings]` section. If there is no such section, the defaults are used. If we were to add all the default bindings to this file it would look something like the following

```toml
[bindings]
img_next = ["d", "right"]
img_prev = ["a", "left"]
img_orig = ["q"]
img_fit = ["f"]
img_del = ["delete"]
pan = ["space"]
play_anim = ["alt+a", "alt+v"]
play_present = ["p"]
play_present_rnd = ["alt+p"]
toggle_fullscreen = ["F11"]
escape = ["Escape"]
```

Note that all items in this section are optional so it's fully valid to only specify one of the actions. In this case all the rest will use the default bindings. For example

```toml
[bindings]
img_next = ["space", "right"]
pan = []
```

The names of the actions _are_ case sensitive but the input strings are _not_.

It is valid to specify an empty array like `img_del = []` in which case the action will never be triggered.

A config file with bindings will look like the following.

```toml
[bindings]
img_next = ["d", "right"]
img_prev = ["a", "left"]
img_orig = ["q"]
img_fit = ["f"]
img_del = ["delete"]
pan = ["space"]
play_anim = ["alt+a", "alt+v"]
play_present = ["p"]
play_present_rnd = ["alt+p"]
```

## Modifiers

Modifiers may be specified separated by '+' characters. For example `"ctrl+x"` or `"ctrl+alt+u"`. Spaces are trimmed from each element and so
`" ctrl+ x"` or `"ctrl + alt+u  "` are equally valid.

The following modifiers are valid

- `alt`: The alt key
- `ctrl`: The control key
- `logo`: The Windows key (Windows) or the Command key (MacOS)

## Keys

Everything typeable is supported including unicode characters like `ø`, `ű`, and `💜`.

There are two special cases for typeable characters:
- `' '` must be specified as `space`
- `'+'` must be specified as `add`.

The following list contains all supported non-typeable keys' names.

```
/// The Escape key, next to F1.
Escape,

F1,
F2,
F3,
F4,
F5,
F6,
F7,
F8,
F9,
F10,
F11,
F12,
F13,
F14,
F15,
F16,
F17,
F18,
F19,
F20,
F21,
F22,
F23,
F24,

/// Print Screen/SysRq.
Snapshot,
/// Scroll Lock.
Scroll,
/// Pause/Break key, next to Scroll lock.
Pause,

/// `Insert`, next to Backspace.
Insert,
Home,
Delete,
End,
PageDown,
PageUp,

Left,
Up,
Right,
Down,

/// The Backspace key, right over Enter.
Back,
/// The Enter key.
Return,

/// The "Compose" key on Linux.
Compose,

Numlock,
Numpad0,
Numpad1,
Numpad2,
Numpad3,
Numpad4,
Numpad5,
Numpad6,
Numpad7,
Numpad8,
Numpad9,

Apps,
Ax,
Calculator,
Capital,
Convert,
Decimal,
Kana,
Kanji,
LAlt,
LControl,
LShift,
LWin,
Mail,
MediaSelect,
MediaStop,
Mute,
MyComputer,
NavigateForward,  // also called "Prior"
NavigateBackward, // also called "Next"
NextTrack,
NoConvert,
OEM102,
PlayPause,
Power,
PrevTrack,
RAlt,
RControl,
RShift,
RWin,
Sleep,
Stop,
Sysrq,
Unlabeled,
VolumeDown,
VolumeUp,
Wake,
WebBack,
WebFavorites,
WebForward,
WebHome,
WebRefresh,
WebSearch,
WebStop,
Yen,
Copy,
Paste,
Cut,
```
