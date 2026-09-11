#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

export GDK_BACKEND=x11
export GSK_RENDERER=cairo
export GTK_A11Y=none
export GIO_USE_VFS=local
# Even GTK's platform integrations cannot reach the host system bus.
export DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent-systemd-gtk-test-bus

exec dbus-run-session --config-file=tests/session-bus.conf -- \
  xvfb-run -a -s "-screen 0 1280x1024x24" \
    cargo test --locked --features ui-tests --test ui -- --test-threads=1 "$@"
