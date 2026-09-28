#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
project=${PROJECT:-"$root"}
action=${1:-bundle}
cd "$root"

case $action in
    bundle|--relaunch) ;;
    *)
        echo "usage: $0 [--relaunch]" >&2
        exit 2
        ;;
esac

platform=$(uname -s)
case $platform in
    Darwin)
        formats=${BUNDLE_FORMATS:-app}
        ;;
    Linux)
        formats=${BUNDLE_FORMATS:-deb}
        ;;
    *)
        echo "Farcaster bundles support macOS and Linux" >&2
        exit 1
        ;;
esac

if [ "$platform" != "Darwin" ] && [ "$action" = "--relaunch" ]; then
    echo "bundle-relaunch supports macOS" >&2
    exit 2
fi

mkdir -p "$target_dir/release"
# cargo-packager resolves file paths relative to its config in packaging/.
target_dir=$(CDPATH= cd -- "$target_dir" && pwd)
if [ "$platform" = "Linux" ]; then
    CARGO_TARGET_DIR="$target_dir" cargo build --release --locked --bin farcaster
    launcher="$target_dir/release/io.github.behzade.farcaster"
    cat >"$launcher" <<'EOF'
#!/bin/sh
exec "$(dirname "$0")/farcaster" "$@"
EOF
    chmod 755 "$launcher"
    cp "$launcher" "$launcher."

    packager_config="$root/packaging/linux.toml"

    unset SOURCE_DATE_EPOCH
    cargo packager --config "$packager_config" --formats "$formats" \
        --out-dir "$target_dir/release" --binaries-dir "$target_dir/release"
else
    CARGO_TARGET_DIR="$target_dir" cargo packager --release --formats "$formats" \
        --out-dir "$target_dir/release"
fi

production_bundle_identifier=io.github.behzade.farcaster
bundle_identifier=$production_bundle_identifier
if [ "$platform" = "Darwin" ]; then
    bundle="$target_dir/release/Farcaster.app"
    if [ ! -d "$bundle" ]; then
        echo "macOS bundle not found: $bundle" >&2
        exit 1
    fi
    identity=${CODESIGN_IDENTITY:--}
    if [ "$identity" = "-" ]; then
        bundle_identifier=$bundle_identifier.dev
        /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $bundle_identifier" \
            "$bundle/Contents/Info.plist"
        codesign --force --sign - "$bundle/Contents/MacOS/farcaster"
        codesign --force --sign - \
            --requirements "=designated => identifier \"$bundle_identifier\"" "$bundle"
    else
        codesign --force --options runtime --timestamp --sign "$identity" \
            "$bundle/Contents/MacOS/farcaster"
        codesign --force --options runtime --timestamp --sign "$identity" "$bundle"
    fi
    codesign --verify --deep --strict "$bundle"
fi

if [ "$action" != "--relaunch" ]; then
    exit 0
fi

case $platform in
    Darwin)
        for identifier in "$production_bundle_identifier" "$bundle_identifier"; do
            osascript -e "if application id \"$identifier\" is running then tell application id \"$identifier\" to quit" \
                >/dev/null 2>&1 || true
        done
        app_is_running() {
            osascript -e "application id \"$1\" is running" 2>/dev/null | grep -q true
        }
        attempts=0
        while app_is_running "$production_bundle_identifier" \
            || app_is_running "$bundle_identifier"; do
            if [ "$attempts" -ge 50 ]; then
                echo "Farcaster did not stop within five seconds" >&2
                exit 1
            fi
            attempts=$((attempts + 1))
            sleep 0.1
        done
        open -n "$bundle" --args "$project"
        ;;
esac
