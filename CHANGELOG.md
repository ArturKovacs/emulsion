## 7.2 on 2020-11-30

### Changed
- Fixed the issue that the first image was shown from the folder after deleting an image instead of the next.
- Fixed a crash for when trying to open an image with invalid EXIF orientation.

## 6.0 on 2020-10-08

### Added
- The currently shown image can now be copied to the clipboard with a keyboard shortcut.
- Image files without a filename extension can now be opened.
- The image antialiasing (filtering) mode can now be manually toggled.

### Changed
- Fixed an issue that caused the images to be loaded from the hard drive even if they were already loaded and avaiable in the cache.
- Fixed an issue that prevented some images from correctly fitting inside the viewport.

## 5.0 on 2020-08-17

### Added

- AVIF support
- Supported MIME types are now added to the `.destkop` file for the linux release; so the file manager should offer up Emulsion for any supported filetype before applications that don't support said filetype.

### Changed

- Panning can now be done with Left Click (instead of Right Click)
- Now there are limits on panning and zooming to avoid getting the image "too far from the view area"
- Some images that were shown with an incorrect orientation are now shown corrently.
- Opening a file through the CLI in the current folder does not need a leading dot-slash (./) anymore
- Fixed an issue where sometimes one image would get stuck preventing the user from switching to another image from the folder.

## 4.0 on 2020-06-23

### Changed
- Image files are now ordered case-insensitively

## 3.0 on 2020-06-21

### Added
- Linux .deb package
- Animated PNG support
- Support for opening directories (both drag & drop and cli argument)
- Best fit mode that displays images at their original size when they fit into the window instead of stretching them.

### Changed
- Changed the UI layout to accomodate the scaling mode buttons.
- Directory contents and the current file is refreshed when the Emulsion window gains focus.
- ~~Image files are now ordered case-insensitively~~
- Significantly decreased CPU and GPU usage.
- Instead of the description, the program name is shown in many context on Windows.
- Many images that couldn't be opened with Emulsion now can be.
- The '1' key can now by default be used for setting the scaling mode to "original size"
- The Return key can now by default be used to toogle full-screen mode.

## 2.1 on 2020-05-23

### Added
- Custom commands to execute on the current image.
- Command line arguments on Linux/macOS to print the current version and the search location of the config file.

### Changed
- Better display quality for certain large images.

## 2.0 on 2020-05-02

### Added
- Animated gif support.
- Custom key-bindings.
- Config entry to disable update-checks.
- The playback or presentation state is now displayed in the titlebar.

### Changed
- Pressing escape in full-screen mode will go back to windowed mode instead of exiting.
- Improved legibility of the info screen.
- Made the dark shade even darker.
- When compiling from source, networking is an optional feature.

### Fixed
- The windows installer won't replace the Adobe Reader DC icon.
- Fixed hang when starting up on Wayland.

## 1.9 on 2020-04-21

Baseline for the changelog.
