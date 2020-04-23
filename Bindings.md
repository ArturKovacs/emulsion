
## General

The names of the actions _are_ case sensitive but the input strings are _not_.

It is valid to specify an empty array like `img_del = []` in which case the action will never be triggered.

A config file with bindings will look like the following.

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

## Modifiers

Modifiers may be specified separated by '+' characters. For example `"ctrl+x"` or `"ctrl+alt+u"`. Spaces are trimmed from each element and so
`" ctrl+ x"` or `"ctrl + alt+u  "` are equally valid.

The following modifiers are valid

- `alt`: The alt key
- `ctrl`: The control key
- `logo`: The Windows key (Windows) or the Command key (MacOS)

## Keys

Everything typeable is supported including unicode characters like `ø`, `ű`, and `💜`

The following list contains all supported non-typeable keys' names (and some of the typeable ones as well).

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

Caret,

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

AbntC1,
AbntC2,
Add,
Apostrophe,
Apps,
At,
Ax,
Backslash,
Calculator,
Capital,
Colon,
Comma,
Convert,
Decimal,
Divide,
Equals,
Grave,
Kana,
Kanji,
LAlt,
LBracket,
LControl,
LShift,
LWin,
Mail,
MediaSelect,
MediaStop,
Minus,
Multiply,
Mute,
MyComputer,
NavigateForward,  // also called "Prior"
NavigateBackward, // also called "Next"
NextTrack,
NoConvert,
NumpadComma,
NumpadEnter,
NumpadEquals,
OEM102,
Period,
PlayPause,
Power,
PrevTrack,
RAlt,
RBracket,
RControl,
RShift,
RWin,
Semicolon,
Slash,
Sleep,
Stop,
Subtract,
Sysrq,
Tab,
Underline,
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