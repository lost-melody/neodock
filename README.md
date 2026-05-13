# NeoDock

A simple _App Dock_ for the _Niri_ compositor,
built with [gtk-rs](https://gtk-rs.org).

## Requirements

- [Niri](https://github.com/niri-wm/niri), a scrollable-tiling _Wayland_ compositor.
- [libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/), building blocks for modern _GNOME_ applications.
- [Tela icon theme](https://github.com/vinceliuice/Tela-icon-theme), a flat colorful design icon theme.

## Screenshots

<details>
<summary>Dark</summary>

![neodock-dark.png](https://i.postimg.cc/j2YQB9P0/neodock-dark.png)
![neodock-dark-menu.png](https://i.postimg.cc/d3cmgXdX/neodock-dark-menu.png)

</details>

<details>
<summary>Light</summary>

![neodock-light.png](https://i.postimg.cc/gjbq15R7/neodock-light.png)
![neodock-light-menu.png](https://i.postimg.cc/kGPFLZ8L/neodock-light-menu.png)

</details>

## Build

Build release binary:

```sh
# build release binary to `./target/release/`.
cargo build --release
```

Run debug binary:

```sh
# build debug binary to `./target/debug/` and run it.
cargo run
```

Install into `~/.cargo/bin`:

```sh
# install from local repository.
cargo install --path .
# install from GitHub.
cargo install --git https://github.com/lost-melody/neodock
```

## Configuration

_NeoDock_ is configured with a config file instead of _GSettings_,
typically `~/.config/io.github.lost-melody.NeoDock/config.toml`.
I believe that by this way the configuration can be easily managed across devices.

Example:

```toml
# filters windows and apps to display.
# valid values:
# - "All"/"all";
# - "SameOutput"/"same_output"/"output";
# - "SameWorkspace"/"same_workspace"/"workspace".
windows_filter = "same_output"
# command to run on launcher button clicked.
launcher_command = [
    "qs", "-c", "noctalia-shell", "ipc", "call", "launcher", "toggle",
]
# list of application ids pinned to dock, where app_id is generally filenames
# in `/usr/share/applications/` without a `.desktop` extension.
pinned_apps = [
    "firefox",
    "kitty",
    "org.gnome.Nautilus",
    "steam",
]
# application id substitution dictionary.
[app_id_substitution]
Chromium = "chromium"
QQ = "com.qq.QQ"
```

Example _Niri_ layer rule:

```kdl
layer-rule {
    match namespace="^neodock$"
    geometry-corner-radius 12
    shadow {
        on
    }
    background-effect {
        blur true
        xray false
    }
    popups {
        geometry-corner-radius 15
        background-effect {
            blur true
        }
    }
}
```

Styles can be overridden by configuring user styles,
typically `~/.config/io.github.lost-melody.NeoDock/style.css`.

```css
/* variables in the scope of dock window. */
.neodock-window {
  --dock-border-color: alpha(var(--headerbar-border-color), 0.25);
  --dock-border-radius: 12px;
  --dock-peek-border-radius: 4px;
  --dock-view-padding: 0.25em;
  --dock-view-min-width: 24em;
  --dock-icon-margin: 0.25em;
  --dock-transition-duration: 0.25s;
  --dock-animation-duration: var(--dock-transition-duration);
}
/* popover menu's background. */
popover.menu > contents {
  background-color: alpha(var(--view-bg-color), 0.3);
}
```

> See `/path/to/repo/resources/css/style.css`.
