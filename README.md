# Systemd GTK

> **⚠️ Warning:** This app is not yet production-ready and has undergone limited testing. Use it at your own risk!

## Description

This application provides a graphical user interface (GUI) for common [systemd](https://systemd.io/) features.
It is designed to make Linux system management more accessible by reducing reliance on the command line.
The app is lightweight and fast, with a size of approximately 600 kB.

## Features

> **⚠️ Warning:** While browsing and viewing unit files is safe, **destructive actions** (such as enabling, disabling,
> starting, stopping, or restarting units) are marked with **red buttons** and will prompt for the root password. **Only
use these actions if you know what you are doing!**

- **List Units**: Displays all available `systemd` units. Refer to
  the [systemctl list-units documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#list-units).
- **Enable Units**: Allows enabling `systemd` units to start automatically at boot. Refer to
  the [systemctl enable documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#enable%20NAME...).
- **Disable Units**: Prevents `systemd` units from starting automatically at boot. Refer to
  the [systemctl disable documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#disable%20NAME...).
- **Start Units**: Initiates the runtime execution of a unit. Refer to
  the [systemctl start documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#start%20NAME...).
- **Stop Units**: Halts the runtime execution of a unit. Refer to
  the [systemctl stop documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#stop%20NAME...).
- **Restart Units**: Stops and then starts the runtime execution of a unit. Refer to
  the [systemctl restart documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#restart%20NAME...).
- **View Unit File Content**: Displays the configuration of individual unit files. Refer to
  the [systemctl cat documentation](https://www.freedesktop.org/software/systemd/man/systemctl.html#cat%20NAME...).
- **Prompt for Root Permissions**: Automatically requests root permissions through the UI when required for privileged
  actions.

## Architecture

- **UI Framework**: Built with [gtk-rs](https://github.com/gtk-rs/gtk4-rs) for a modern and user-friendly interface.
- **Systemd Interaction**: Uses the [systemctl](https://github.com/gwbres/systemctl) library, which communicates with
  `systemd` via pipes for robust
  functionality.

## Requirements

- [Gnome 44](https://www.gnome.org/) or newer
- [systemd](https://github.com/systemd/systemd)
- [Rust](https://www.rust-lang.org/tools/install)

## Development

1. Ensure your system meets the requirements listed above.
2. Navigate to your dev folder.
3. Clone this repository:
   ```bash
   git clone https://github.com/your-username/systemd-gtk.git
4. Navigate to the project directory:
   ```bash
   cd systemd-gtk
5. Build and run the application:
   ```bash
   cargo run
6. Create an optimized release build:
   ```bash
   cargo build --profile release-lto

## Binaries

Binaries for x64 are available at the [release page](https://github.com/Journeycorner/systemd-gtk/releases).

## Wishlist

* testing!
* publish on [crates.io](https://crates.io/)
* switch to upstream systemctrl dependency, once it was patched
* free memory after viewing unit files
* use more idiomatic Rust: get rid of some .clone() calls
* publish [AUR](https://aur.archlinux.org/)
* publish https://flatpak.org/