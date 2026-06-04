#!/bin/sh
# Install the latest scrolls binary. Usage:
#   curl -fsSL https://raw.githubusercontent.com/kashgohil/scrolls/master/install.sh | sh
set -e

repo="kashgohil/scrolls"
bin="scrolls"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin)
    case "$arch" in
      arm64 | aarch64) target="aarch64-apple-darwin" ;;
    esac
    ;;
  Linux)
    case "$arch" in
      x86_64) target="x86_64-unknown-linux-gnu" ;;
    esac
    ;;
esac

if [ -z "$target" ]; then
  echo "Unsupported platform: $os $arch" >&2
  echo "You can still install with Rust: cargo install --git https://github.com/$repo" >&2
  exit 1
fi

url="https://github.com/$repo/releases/latest/download/$bin-$target.tar.gz"
dir="$HOME/.local/bin"

echo "Downloading $bin for $target..."
tmp=$(mktemp -d)
curl -fsSL "$url" | tar -xz -C "$tmp"
mkdir -p "$dir"
mv "$tmp/$bin" "$dir/$bin"
chmod +x "$dir/$bin"
rm -rf "$tmp"

echo "Installed $bin to $dir/$bin"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Note: add $dir to your PATH, e.g. echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc" ;;
esac
