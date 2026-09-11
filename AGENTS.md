# Working on systemd GTK

## Scope and priorities

This is a Linux desktop manager for loaded system and current-user systemd
units, built with Rust, Relm4, GTK 4, libadwaita, and zbus. Prefer small,
evidence-backed improvements to correctness, usability, and maintainability.
Do not expand into unit editing, journal browsing, remote administration, or
other features without a request. Read README.md and relevant code/tests first.
Preserve unrelated work; never reset the worktree or remove user changes.

## Architecture and Rust conventions

- Keep unit data, action eligibility, search/sort rules, and request generations
  in src/model.rs independent of GTK and D-Bus.
- Keep typed D-Bus calls and file I/O behind SystemdBackend in src/backend/.
  Use owned data and Send futures/streams across that boundary.
- Keep GTK objects on the main thread. Use Relm4 inputs, commands, outputs,
  and controller ownership; never block the UI with I/O or synchronous waits.
- Prefer ordinary structs/enums and existing Relm4 facilities over custom
  GObject types, global mutable state, extra dependencies, or generic frameworks.
- Propagate contextual errors. Avoid unwrap/expect on external data or runtime
  failures; reserve them for genuine invariants and tests. Avoid unsafe code
  unless necessary, justified, and tested.
- Reject stale asynchronous results using scoped identities and request
  generations. Cancel subscriptions and release dialogs when their owner closes.
  Recycled row widgets must reset any state-dependent styling on each bind.

## System administration safety

- Never start, stop, restart, enable, or disable host units during automated
  development or testing. Use the private fake buses and injected backends.
- Run the app as an ordinary user. Delegate authorization to systemd/polkit;
  never collect passwords, shell out to sudo, or add privilege escalation.
- Unit identity includes its scope. Revalidate capabilities before mutation;
  serialize operations and keep their original target through completion.
- Confirm disruptive actions with the target and scope. Enablement and runtime
  state are independent. Never automatically retry an uncertain mutation.
- A timeout is not proof of failure. Track job completion, report partial
  success honestly, and disable mutation controls when state is stale.
- Builds and ordinary startup must not install files into the user's desktop.
  Desktop installation belongs exclusively to packaging, never the application
  binary. Package installation stages into pkgdir.

## UI and accessibility

- Follow GNOME HIG and the installed libadwaita version's documented patterns.
  Prefer native widgets, semantic style classes, and theme-relative colors.
- Keep the dense unit table virtualized, sortable, keyboard-accessible, and
  searchable. Preserve selection by scoped identity across updates.
- Keep state text readable. Color supplements text, never replaces it. Preserve
  native hover, selection, and focus indicators. Review light/dark themes and
  high contrast, empty/error/loading states, and constrained window sizes.
- Distinguish runtime actions from enablement. Scope shortcuts to their intended
  widgets so Enter and Escape keep working in dialogs and text inputs.
- Keep APP_ID, installed desktop basename, and icon name consistent. Embed
  runtime assets so cargo run and installed binaries do not depend on the cwd.
- Consult primary GTK/Relm4/Adwaita documentation when APIs or behavior are
  uncertain. Record substantive presentation decisions in docs/design.md.

## Build and packaging defaults

- Use the pinned stable toolchain and Rust edition in the manifest. Keep
  Cargo.lock committed; update dependencies deliberately and verify changes.
- Use the standard release profile: ThinLTO and stripped symbols. Do not add a
  separate release profile merely to enable optimization.
- Repository rustflags default to target-cpu=native. This is for local builds,
  not portable binary distribution; use an explicit baseline when distributing.
  Native tuning applies to debug builds too. Git/registry cargo install ignores
  repository config; do not claim those installs automatically use native.
- Keep local Arch source snapshots, AUR recipe, Cargo asset inclusion, desktop
  entry, and documentation in sync. Retain .cargo/config.toml in local snapshots.
  Regenerate build/aur/.SRCINFO after recipe metadata changes.
- Do not publish crates, push commits, submit AUR updates, or install system
  packages unless the user authorizes those actions. Commit only when requested.

## Verification

Run checks proportionate to changes. Before a broad handoff, use:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
bash scripts/test-ui.sh
cargo build --locked --release
bash scripts/test-packaging.sh
git diff --check
```

Add regression tests for changed behavior. Use pure tests for model rules,
isolated D-Bus integration tests for protocol behavior, and the single-threaded
headless GTK workflow for component interactions. Prefer observable conditions
with deadlines over arbitrary sleeps. Include stale results, errors, duplicate
requests, disconnects, and lifecycle cleanup when relevant.

Packaging tests must use temporary destinations, not the real user data
directory. Check cargo install with a temporary --root when changing installation.
Never claim real polkit prompts or shell/dock integration were verified by mocks;
use docs/testing.md for manual checks in a disposable environment. Report the
checks actually run and any limitations, and keep README.md accurate.
