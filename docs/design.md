# Presentation and integration decisions

This is a dense unit inspector, not a settings panel. Keep the virtualized,
sortable GTK ColumnView instead of replacing thousands of units with cards.
Use compact `data-table` styling, plain row backgrounds, an explicit sortable
Type column, and separators in high contrast. Alternating shading was removed
after visual feedback because it distracted from the data. Type labels expose
the unit-name suffix (including unfamiliar types) without additional colors.
Preserve native selected,
hovered, and focused row styles. Inactive units remain fully readable; state
labels use Adwaita semantic colors for active/failed and always retain text.
Color is supplementary, never the only state indicator.

Runtime actions and enablement are distinct: spacing and tooltips make clear
that enabling does not start a unit and disabling does not stop it. Enter is
handled by the focused widget, not intercepted by a global accelerator that
could interfere with confirmation dialogs.

The generated app icon is embedded as a GResource. GTK can resolve it with no
installation or checkout-relative path. The shell still needs a desktop entry:
the application ID, installed desktop basename, and icon name all use
`com.journeycorner.systemd-gtk`. Desktop installation belongs to packaging;
the application contains no installer. Neither build.rs nor application startup
writes desktop files. Cargo alone installs only the executable; use the Arch
package for a complete desktop installation.

Sources consulted:

- [GNOME list and column views](https://developer.gnome.org/hig/patterns/containers/list-column-views.html): large dynamic lists, regular rows and sortable column headers.
- [GTK ColumnView](https://docs.gtk.org/gtk4/class.ColumnView.html): virtualized columns and CSS nodes.
- [Adwaita style classes](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.9/style-classes.html): semantic colors, typography, linked controls, contrast-aware styling.
- [Adwaita ColumnView stylesheet](https://github.com/GNOME/libadwaita/blob/main/src/stylesheet/widgets/_column-view.scss): `data-table` reduces padding; it does not provide stripes.
- [GNOME app-icon guidance](https://developer.gnome.org/hig/guidelines/app-icons): simple recognizable silhouettes and restrained depth.
- [GNOME application IDs](https://developer.gnome.org/documentation/tutorials/application-id.html): consistent desktop identity.
- [Cargo installation](https://doc.rust-lang.org/cargo/commands/cargo-install.html): binary installation and lockfile behavior.
- [Desktop Entry Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html): executable quoting and field codes.
- [Arch PKGBUILD manual](https://man.archlinux.org/man/PKGBUILD.5.en): source staging, check/package phases and VCS recipes.

## Icon provenance

Created with the imagegen skill using the built-in image-generation tool, not
the API/CLI fallback. Final asset: `data/app-icon.png`. The original transparent
PNG is preserved; its resource/theme directory specifies its nominal icon size.
The former SVG is retained as an unused historical asset.

Final generation prompt:

```text
Use case: logo-brand
Asset type: full-color GNOME desktop application icon for systemd-gtk, a system and user service manager.
Primary request: a polished, simple icon depicting a compact blue control cabinet with three stacked rounded service modules and a small green status indicator. Strong unified silhouette, broad uncluttered shapes, subtle GNOME-style depth with a shallow darker base. Straight-on view, not isometric.
Composition: one centered object on a genuinely transparent square background, generous padding, readable at 32 and 128 pixels. No surrounding tile, no text, no letters, no logos, no watermark. Preserve alpha transparency. Output PNG.
```
