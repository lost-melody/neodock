# NeoDock

A simple _App Dock_ for the _Niri_ compositor,
built with [gtk-rs](https://gtk-rs.org).

## Requirements

- [Niri](https://github.com/niri-wm/niri), a scrollable-tiling _Wayland_ compositor.
- [libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/), building blocks for modern _GNOME_ applications.
- [Tela icon theme](https://github.com/vinceliuice/Tela-icon-theme), a flat colorful design icon theme.

## Build

If you have [task](https://taskfile.dev) installed, run:

```sh
# gresource is required for building.
task resources
# builds a release binary at `./target/release/neodock`.
task release
```

This is equivalent to running:

```sh
glib-compile-resources resources.gresource.xml --sourcedir ./ --target ../target/resources.gresource
cargo build --release
```

> See `Taskfile.yml`.

## Configuration

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
  --dock-border-color: var(--border-color);
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
