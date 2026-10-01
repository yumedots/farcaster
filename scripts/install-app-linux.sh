#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
id=io.github.behzade.farcaster
data=${XDG_DATA_HOME:-"$HOME/.local/share"}
bin_dir=${XDG_BIN_HOME:-"$HOME/.local/bin"}

CARGO_TARGET_DIR="$target_dir" cargo build --release --locked --bin farcaster

mkdir -p "$bin_dir" "$data/applications" "$data/icons/hicolor/512x512/apps"
cp "$target_dir/release/farcaster" "$bin_dir/farcaster"
cp "$root/assets/icons/app/icon_512x512.png" "$data/icons/hicolor/512x512/apps/$id.png"
cat >"$data/applications/$id.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Farcaster
Exec=$bin_dir/farcaster %F
Icon=$id
Categories=Development;
Terminal=false
EOF
echo "installed $bin_dir/farcaster"
echo "installed $data/applications/$id.desktop"
