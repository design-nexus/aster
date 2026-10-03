#!/usr/bin/env bash
# Build starship-editor and install the binary.
# From a clone:  ./install.sh
# From the web:  curl -fsSL https://raw.githubusercontent.com/design-nexus/starship-editor/main/install.sh | bash
# Optional:      ./install.sh --prefix "$HOME/.local"

set -euo pipefail

PREFIX="${HOME}/.local"
REPO="https://github.com/design-nexus/starship-editor.git"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="${2:?--prefix needs a directory}"
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#--prefix=}"
      shift
      ;;
    -h|--help)
      echo "Usage: install.sh [--prefix DIR]"
      echo "Installs the starship-editor binary into DIR/bin. Default DIR is ~/.local."
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found. Install Rust from https://rustup.rs and run this again." >&2
  exit 1
fi

cleanup() {
  if [[ -n "${CLONE_DIR:-}" ]]; then
    rm -rf "$CLONE_DIR"
  fi
}
trap cleanup EXIT

source_dir=""
script_path="${BASH_SOURCE[0]:-}"
if [[ -n "$script_path" && -f "$script_path" ]]; then
  candidate="$(cd "$(dirname "$script_path")" && pwd)"
  if [[ -f "$candidate/Cargo.toml" ]] && grep -q 'name = "starship-editor"' "$candidate/Cargo.toml"; then
    source_dir="$candidate"
  fi
fi

if [[ -z "$source_dir" ]]; then
  if ! command -v git >/dev/null 2>&1; then
    echo "git was not found, and this script is not inside a starship-editor checkout." >&2
    exit 1
  fi
  CLONE_DIR="$(mktemp -d)"
  echo "Cloning $REPO"
  git clone --depth 1 "$REPO" "$CLONE_DIR/starship-editor"
  source_dir="$CLONE_DIR/starship-editor"
fi

echo "Building release binary"
cargo build --release --manifest-path "$source_dir/Cargo.toml"

install -d "$PREFIX/bin"
dest="$PREFIX/bin/starship-editor"
# A leftover symlink pointed at a removed Omarchy plugin. install(1) would
# follow it and fail, so drop a symlink before copying the binary.
if [[ -L "$dest" ]]; then
  rm -f "$dest"
fi
install -m 755 "$source_dir/target/release/starship-editor" "$dest"

old="$PREFIX/bin/omarchy-starship"
if [[ -L "$old" && ! -e "$old" ]]; then
  rm -f "$old"
fi

echo "Installed $dest"
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *)
    echo "Add this to your shell profile if that directory is not already on PATH:"
    echo "  export PATH=\"$PREFIX/bin:\$PATH\""
    ;;
esac
echo "Run: starship-editor"
