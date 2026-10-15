#!/usr/bin/env bash
# Build the Deck Calendar widget and install it on a Braiins Deck.
#
# Usage: ./deploy.sh <deck-ip>
#
# Packages the widget the way the Deck firmware's own build does, then adds
# it with the Deck's package manager, like the SDK's `nix run .#deck --
# deploy` but without Nix. Re-running replaces the installed version; remove
# it with ./undeploy.sh.
set -euo pipefail

DECK_IP="${1:?usage: ./deploy.sh <deck-ip>}"
WIDGET=deck-calendar
HERE="$(cd "$(dirname "$0")" && pwd)"
SDK="$HERE/bmc-sdk"
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 "root@$DECK_IP")
# Homebrew's rustup is keg-only, so it may not be on PATH.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

if [ ! -f "$SDK/Cargo.toml" ]; then
	echo "The SDK is missing; run: git submodule update --init" >&2
	exit 1
fi
echo "==> Checking the Deck at $DECK_IP"
if ! "${SSH[@]}" true; then
	echo "Cannot log in to root@$DECK_IP; set up key login once with: ssh-copy-id root@$DECK_IP" >&2
	exit 1
fi

manifest="$HERE/$WIDGET/manifest.json"
field() { python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$manifest" "$1"; }
version=$(field version)
icon=$(field icon)
wasm_name="$(tr - _ <<<"$WIDGET").wasm"

echo "==> Building $WIDGET $version"
cargo build --manifest-path "$HERE/Cargo.toml" -p "$WIDGET" --release --target wasm32-unknown-unknown
release="$HERE/target/wasm32-unknown-unknown/release"
assets_tool="$SDK/target/debug/bmc-wasm-assets"
if [ ! -x "$assets_tool" ]; then
	echo "==> Building the SDK's packaging tool (first run only)"
	cargo build --manifest-path "$SDK/Cargo.toml" -p bmc-wasm-assets
fi

echo "==> Packaging"
work="$HERE/.tmp/deploy/$WIDGET"
package="bmc-widget-$WIDGET"
base="$work/$package/lib/bmc-widgets/$WIDGET"
rm -rf "$work"
mkdir -p "$work/stage" "$base/bin" "$base/lib/wasm" "$base/lib/assets"
# Split compiled assets out of the wasm into their own files, as the firmware build does.
"$assets_tool" extract \
	--input "$release/$wasm_name" \
	--wasm-output "$work/stage/$wasm_name" \
	--asset-root "$work/stage/assets" \
	--artifact-root "$release/deps"
"$assets_tool" verify-stripped --input "$work/stage/$wasm_name"
cp "$work/stage/$wasm_name" "$base/lib/wasm/$WIDGET.wasm"
if [ -d "$work/stage/assets" ]; then
	cp -R "$work/stage/assets/." "$base/lib/assets/"
fi
cp "$manifest" "$base/manifest.json"
mkdir -p "$(dirname "$base/$icon")"
cp "$HERE/$WIDGET/$icon" "$base/$icon"
# The firmware's wrappers name their own store path; this one is added to the
# store by content, so it finds its files relative to itself instead.
cat >"$base/bin/$WIDGET" <<EOF
#!/bin/sh
here=\$(dirname "\$(readlink -f "\$0")")
exec /run/current-profile/bin/bmc-wasm-thin-v0 \\
  --wasm "\$here/../lib/wasm/$WIDGET.wasm" \\
  --asset-root "\$here/../lib/assets" \\
  "\$@"
EOF
chmod 755 "$base/bin/$WIDGET"

echo "==> Installing on the Deck"
COPYFILE_DISABLE=1 tar --format ustar --no-xattrs -cf - -C "$work" "$package" |
	"${SSH[@]}" "rm -rf /tmp/$package && tar -xf - -C /tmp"
"${SSH[@]}" sh -s -- "$WIDGET" "$version" <<'EOF'
set -e
widget=$1
version=$2
export PATH=/run/current-profile/bin:$PATH
path=$(nix-store --add "/tmp/bmc-widget-$widget")
rm -rf "/tmp/bmc-widget-$widget"
/nix/var/nix/gcroots/profiles/bmc/current/bin/bmc-nix-cli add-packages \
	--name "widget-$widget" --version "$version" --store-path "$path"
test -f "/run/current-profile/lib/bmc-widgets/$widget/manifest.json"
EOF

echo "==> Done. Add \"$(field name)\" to a scene in the Deck's web interface."
