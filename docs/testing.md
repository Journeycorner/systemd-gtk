# Testing and desktop validation

## Automated coverage

`cargo test --locked` covers:

- Case-insensitive search and unit-type/name ordering.
- Enablement independent of runtime state, failed-unit starts, unknown states,
  capability restrictions, and pending jobs.
- Request generations, scope isolation, and duplicate-operation prevention.
- Command-line help/version without a display, and rejection of the removed
  self-installation option without writing desktop files.
- Actual D-Bus serialization against private fake Manager and Unit services.
- Start/stop/restart arguments and interactive-authorization flags.
- Job completion emitted before the method reply, unrelated jobs, failed jobs,
  authorization denial, and jobs which never complete.
- Persistent and runtime-only disablement, enablement, reload, and partial
  success when a reload is denied.
- Live signals, connection loss, reset, read timeouts, and fragment/drop-in
  reads, including missing files.

`bash scripts/test-ui.sh` adds the component workflow test: search and sorting,
selection after list replacement, live refreshes, dialogs and memory release,
file errors/retry, mutation serialization, delayed snapshots across scope
changes, confirmation cancellation/acceptance, a 3,000-unit snapshot, and
connection recovery. No test controls real units.
It also checks embedded icon lookup, stale capabilities during reconnect,
and event streams ending without an explicit disconnection event.

Set `SYSTEMD_GTK_SCREENSHOTS=target/ui-screenshots` when running the UI script
to save light and dark PNG captures for visual review.

After a release build, `bash scripts/test-packaging.sh` checks desktop validation,
the staged binary/icon/launcher, local source snapshot creation, and Cargo's
package file list. Its temporary outputs are printed and retained for inspection;
it does not install a system package or register a launcher in your real home.

## Manual desktop checklist

Use a disposable VM with the required GTK libraries and a desktop authentication
agent. Automated tests do not verify real polkit dialogs or desktop rendering.

1. Launch `cargo run --locked` as an ordinary desktop user. Check both manager
   scopes, search, column sorting, refresh, and keyboard shortcuts.
2. Check light, dark, and high-contrast themes. Resize to 800×600, verify table
   scrolling and readable controls, and navigate using only the keyboard.
3. View a unit fragment with multiple drop-ins. Close and reopen the dialog.
   Check a unit with no fragment and an unreadable file.
4. Create disposable test services in the VM: one system service and one
   current-user service, each with a harmless long-running command and an
   `[Install]` section. Never substitute an essential service.
5. Exercise start, stop, restart, enable, and disable. Verify the named scope
   and target in confirmations. Confirm enabling does not start the service,
   disabling does not stop it, and changes appear after the manager reload.
6. For system actions, test successful authentication, cancellation, denied
   authorization, and an unavailable authentication agent. The UI should remain
   responsive and display the error without claiming success.
7. Change those test services outside the app and verify live updates preserve
   the current search, sorting, and selected unit.
8. Close the app while a test action is pending. The app should exit cleanly;
   already submitted systemd jobs are not cancelled. In the VM, stop/disable
   and remove the disposable test services when finished.

Do not mark these manual scenarios as completed based on the mock test suite.
