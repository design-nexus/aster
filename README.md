# starship-editor

A terminal wizard for designing a [Starship](https://starship.rs) prompt. You choose the shape, the lines, which pieces appear, and a color for each piece. The preview is the real prompt from `starship prompt`.

It reads `$STARSHIP_CONFIG`, or `~/.config/starship.toml`. Comments, module symbols, and `[os.symbols]` stay unless you ask for plain words or no icons. Nothing is written until you save. The first save copies the current file to `starship.toml.bak.<timestamp>` beside it.

## Install

Rust and Starship are required.

```bash
git clone https://github.com/design-nexus/starship-editor.git
cd starship-editor
./install.sh
```

Or in one step:

```bash
curl -fsSL https://raw.githubusercontent.com/design-nexus/starship-editor/main/install.sh | bash
```

The binary is installed to `~/.local/bin/starship-editor`. Use `--prefix` for another directory.

```bash
cargo build --release
./target/release/starship-editor
```

## Wizard

`starship-editor` opens on the wizard. `--edit` opens the module editor. Every step prints what it decides and which keys work. Esc, or `b`, goes back. On the first step that quits, and nothing has been saved. `q` quits without saving.

1. **Shape.** Plain text, powerline blocks, or capsules. Each row is drawn.
2. **Lines.** One line, two lines, or two lines with a blank line before them.
3. **Info.** Space includes a piece. `[` and `]` reorder it. The prompt character stays last.
4. **Colors.** Left and right change a piece's color. `c` types a hex color. `t` flips the text light or dark.
5. **Icons.** Keep the icons in your file, switch to plain words, or drop icons.
6. **Save.** `s` writes the file. `e` opens the module editor without saving.

## Module editor

The list is Prompt, OS symbols, then every Starship module. `/` filters. Tab moves fields. Enter edits the highlighted field. An empty field removes that key. Space disables a module, or toggles `add_newline` on the prompt. Enter on OS symbols opens the OS list.

`s` saves. `q` asks before discarding unsaved edits. Comments and keys the editor does not touch stay as they are.

The colors follow the live Omarchy theme at `~/.local/state/omarchy/current/theme/colors.toml`, then `~/.config/omarchy/current/`. Otherwise the terminal's own colors are used.
