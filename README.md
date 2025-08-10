# wanipaper

Wanipaper is a wallpaper utility for wlroots-based Wayland compositors.
It supports TOML configuration, per-output customization, and grouping outputs to display cross-monitor wallpapers.
Images can be resized using cover or stretch modes, and wallpapers can change randomly or rotate on a timer.

## Features

* Per-output customization - set different wallpapers for each display
* Output grouping - span wallpapers across multiple monitors
* Resize modes - cover or stretch images to fit outputs or groups
* Wallpaper rotation - random selection, timed cycling, or both

## Usage

```sh
cargo build --release
./target/release/wanipaper
```

Dependencies are listed in the `shell.nix`.

## Configuration

Wanipaper uses a config file located at: `~/.config/wani/wanipaper.config`.

Add displays by assigning the output name to an identifier.
Output names are as reported by `hyprctl monitors all` or similar.
```toml
[displays.primary]
name = "DP-1"

[displays.secondary]
name = "DP-2"
```

Collect multiple displays into a group.
```toml
[groups.all]
displays = ["primary", "secondary"]
```

Load images by assigning them to an identifier.
Loading multiple images will use more memory, but any unused images are unloaded.
```toml
[images.coastline]
path = "coastline.png" # Path relative to config directory
```

Create one or more render passes from source to target.
Cover preserves aspect ratio but crops edges,
Stretch fills target ignoring aspect ratio.
```toml
[[renderpass]]
source = "coastline"
target = "all" # Target can be a group or display name
resize = "cover" # or "stretch"
```

Randomise or rotate through wallpapers with selection, or both at the same time.
```toml
[[renderpass]]
source = ["coastline", "meadow"]
selection.rand = true
# selection.rotate = 120
target = "all"
resize = "cover"
```
