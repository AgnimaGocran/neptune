#!/usr/bin/env bash
# Wrap a built macOS executable in Neptune.app with its Finder/Dock icon.
# Usage: package-macos.sh [--binary PATH] [--output PATH]

set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$(dirname -- "${BASH_SOURCE[0]}")/package-common.sh"

binary=$REPO/target/release/neptune
output=$REPO/target/release/Neptune.app
while [[ $# -gt 0 ]]; do
	case $1 in
	--binary)
		binary=${2:?}
		shift 2
		;;
	--output)
		output=${2:?}
		shift 2
		;;
	*)
		echo "usage: package-macos.sh [--binary PATH] [--output PATH]" >&2
		exit 2
		;;
	esac
done

if [[ ! -f $binary ]]; then
	echo "Build the macOS neptune binary first" >&2
	exit 1
fi
if [[ -e $output ]]; then
	echo "Output already exists; choose a fresh --output directory" >&2
	exit 1
fi

binary_directory=$(dirname -- "$binary")
contents=$output/Contents
mkdir -p "$contents/MacOS" "$contents/Resources"
install -m755 "$binary" "$contents/MacOS/neptune"
cp -p "$binary_directory/neptune-browser" "$contents/MacOS/neptune-browser"
browser_payload "$binary_directory" "$contents/Frameworks" macos

# The five browser helpers share the browser executable; only their bundle
# metadata differs, and each stays a background agent (LSUIElement).
for suffix in "" " (Renderer)" " (GPU)" " (Plugin)" " (Alerts)"; do
	name="Neptune Browser Helper$suffix"
	helper=$contents/Frameworks/$name.app/Contents
	mkdir -p "$helper/MacOS"
	cp -p "$binary_directory/neptune-browser" "$helper/MacOS/$name"
	identifier="rs.neptune.browser.helper$(printf '%s' "$suffix" | tr -d ' ' | sed 's/(/./g; s/)/./g' | tr '[:upper:]' '[:lower:]')"
	cat >"$helper/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>$name</string>
	<key>CFBundleIdentifier</key>
	<string>$identifier</string>
	<key>CFBundleName</key>
	<string>$name</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>LSUIElement</key>
	<true/>
</dict>
</plist>
EOF
done

install -m644 "$REPO/packaging/cef-LICENSE.txt" "$contents/Resources/CEF-LICENSE.txt"
install -m644 "$REPO/assets/icons/neptune.icns" "$contents/Resources/neptune.icns"
version=$(cargo metadata --locked --manifest-path "$REPO/Cargo.toml" --format-version 1 |
	jq -r '.packages[] | select(.name == "neptune-terminal") | .version')
# Apple requires numeric bundle versions. Full SemVer remains in the binary,
# release tag, DMG filename and authenticated update metadata.
bundle_version=${version%%-*}
bundle_version=${bundle_version%%+*}
cat >"$contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDisplayName</key>
	<string>Neptune</string>
	<key>CFBundleExecutable</key>
	<string>neptune</string>
	<key>CFBundleIconFile</key>
	<string>neptune.icns</string>
	<key>CFBundleIdentifier</key>
	<string>rs.neptune.terminal</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Neptune</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$bundle_version</string>
	<key>CFBundleVersion</key>
	<string>$bundle_version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF
printf 'APPL????' >"$contents/PkgInfo"
