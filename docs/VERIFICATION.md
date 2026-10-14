# Verification

Local verification on 2026-10-13:

- `cargo fmt --check`, `cargo test --locked` (31 tests), and `cargo build --release --locked` passed.
- The PTY harness passed navigation, resize transitions, input precedence, staged cancellation, guarded quit, suspension/resume, termios restoration, alternate-screen restoration, mouse release, and terminal hangup checks.
- Render tests cover every screen and pane at 120×40, 100×30, 80×24, and 60×20, plus small sizes and shrink/grow transitions. They also cover palettes, dialogs, builder steps, invalid inputs, Unicode, and hit-target bounds. Mouse property selection and guarded dialog submission have behavioral checks.
- Tests cover shared drafts, history, staged cancellation, nested custom modules, preset adoption, invalid TOML, external changes, permissions, symlink targets, backups, save failures, unknown metadata, custom-command blocking, timeouts, and running-process cancellation.
- Actual Foot captures of every screen and pane and representative overlays at the four required sizes are in `docs/qa/`. Representative wide and compact images were inspected against `TUI_STYLE.md` and Glim's screenshot. README images are actual terminal captures.
- Installation uses the local locked release build. The installed executable is compared byte-for-byte with that build.

## Limits

The last compact-preview adjustment moves scenario metadata below the prompt so that the prompt itself is visible in small panes. The existing compact capture predates that adjustment; the README compact image is therefore a layout reference rather than an exact image of that final adjustment. No further desktop capture was attempted after the user requested a prompt handoff.

This is Linux terminal software. Other operating systems and every terminal emulator have not been tested. Arbitrary custom commands were not executed during verification; approval and blocking were tested with controlled definitions. Full Starship runtime behavior depends on the installed Starship version and the selected directory.

GitHub repository naming and the renamed installer URL were verified. The remote installer still contains the previous code until these local changes are committed and pushed. No commit or push was made, and GitHub CI has not run against these local changes.
