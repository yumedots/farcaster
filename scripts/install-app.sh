#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
bundle="$target_dir/release/Farcaster.app"

if [ ! -d "$bundle" ]; then
    echo "bundle not found: $bundle (run make app first)" >&2
    exit 1
fi

if [ -w /Applications ]; then
    destination=/Applications
else
    destination="$HOME/Applications"
fi

mkdir -p "$destination"
rm -rf "$destination/Farcaster.app"
ditto "$bundle" "$destination/Farcaster.app"
echo "installed $destination/Farcaster.app"
