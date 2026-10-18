#!/usr/bin/env bash
# Developer ID sign, notarize/staple app, then sign/notarize/staple DMG.
#
# Credentials are read only from the GitHub environment and cleaned on exit.
# No ad-hoc signing, deprecated altool, or unsigned fallback.
# Usage: sign-macos.sh --version V --platform {macos-arm64|macos-x64} --binary PATH

set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Run a macOS tool and print its stdout. Never include command arguments
# (keychain/password) in error output.
run() {
	local output
	if ! output=$("$@" 2>/dev/null); then
		echo "$(basename -- "$1") $2 failed" >&2
		return 1
	fi
	printf '%s' "$output"
}

version=""
platform=""
binary=""
while [[ $# -gt 0 ]]; do
	case $1 in
	--version)
		version=${2:?}
		shift 2
		;;
	--platform)
		platform=${2:?}
		shift 2
		;;
	--binary)
		binary=${2:?}
		shift 2
		;;
	*)
		echo "usage: sign-macos.sh --version V --platform {macos-arm64|macos-x64} --binary PATH" >&2
		exit 2
		;;
	esac
done
case $platform in
macos-arm64 | macos-x64) ;;
*)
	echo "usage: sign-macos.sh --version V --platform {macos-arm64|macos-x64} --binary PATH" >&2
	exit 2
	;;
esac

for variable in MACOS_CERTIFICATE_P12_BASE64 MACOS_CERTIFICATE_PASSWORD MACOS_SIGN_IDENTITY APPLE_API_KEY_P8_BASE64 APPLE_API_KEY_ID APPLE_API_ISSUER_ID; do
	if [[ -z ${!variable:-} ]]; then
		echo "Missing macOS release credentials; see docs/releases.md" >&2
		exit 1
	fi
done

dist=$REPO/dist
mkdir -p "$dist"
temporary=$(mktemp -d)
original_keychains=()
keychain=$temporary/release.keychain-db
cleanup() {
	if [[ ${#original_keychains[@]} -gt 0 ]]; then
		security list-keychains -d user -s ${original_keychains[@]+"${original_keychains[@]}"} >/dev/null 2>&1 || true
	fi
	security delete-keychain "$keychain" >/dev/null 2>&1 || true
	rm -rf "$temporary"
}
trap cleanup EXIT

openssl base64 -d -A <<<"${MACOS_CERTIFICATE_P12_BASE64}" >"$temporary/developer-id.p12"
chmod 600 "$temporary/developer-id.p12"
openssl base64 -d -A <<<"${APPLE_API_KEY_P8_BASE64}" >"$temporary/AuthKey.p8"
chmod 600 "$temporary/AuthKey.p8"
keychain_password=$(openssl rand -hex 32)
while IFS= read -r line; do
	original_keychains+=("$(printf '%s' "$line" | sed 's/^"//; s/"$//')")
done < <(run security list-keychains -d user)

run security create-keychain -p "$keychain_password" "$keychain" >/dev/null
run security set-keychain-settings -lut 21600 "$keychain" >/dev/null
run security unlock-keychain -p "$keychain_password" "$keychain" >/dev/null
run security import "$temporary/developer-id.p12" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
run security set-key-partition-list -S "apple-tool:,apple:,codesign:" -s -k "$keychain_password" "$keychain" >/dev/null
run security list-keychains -d user -s "$keychain" ${original_keychains[@]+"${original_keychains[@]}"} >/dev/null

stage=$temporary/volume
mkdir "$stage"
app=$stage/Neptune.app
"$REPO/scripts/package-macos.sh" --binary "$binary" --output "$app"
bash "$(dirname -- "${BASH_SOURCE[0]}")/package-common.sh" notices "$app/Contents/Resources/licenses"

identity=$MACOS_SIGN_IDENTITY
sign_browser() {
	local path=$1 entitlements=${2:-false}
	local arguments=(codesign --force --timestamp --options runtime --keychain "$keychain" --sign "$identity")
	if [[ $entitlements == true ]]; then
		arguments+=(--entitlements "$REPO/packaging/browser-entitlements.plist")
	fi
	run "${arguments[@]}" "$path" >/dev/null
}

frameworks=$app/Contents/Frameworks
framework=$frameworks/Chromium Embedded Framework.framework
while IFS= read -r library; do
	if [[ ! -L $library ]]; then
		sign_browser "$library"
	fi
done < <(find "$framework" -name '*.dylib' | sort)
sign_browser "$framework"
while IFS= read -r helper; do
	while IFS= read -r executable; do
		sign_browser "$executable" true
	done < <(find "$helper/Contents/MacOS" -maxdepth 1 -mindepth 1 | sort)
	sign_browser "$helper" true
done < <(find "$frameworks" -maxdepth 1 -mindepth 1 -name '*.app' | sort)
sign_browser "$app/Contents/MacOS/neptune-browser" true
run codesign --force --timestamp --options runtime --keychain "$keychain" --sign "$identity" "$app/Contents/MacOS/neptune" >/dev/null
run codesign --force --timestamp --options runtime --keychain "$keychain" --sign "$identity" "$app" >/dev/null
run codesign --verify --deep --strict "$app" >/dev/null

archive=$temporary/Neptune.zip
run ditto -c -k --keepParent "$app" "$archive" >/dev/null

# Submit to Apple and wait; anything but acceptance stops the release, and the
# submission id in the message retrieves the notarytool log.
notarize() {
	local path=$1 response
	if ! response=$(run xcrun notarytool submit "$path" --key "$temporary/AuthKey.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID" --wait --timeout 30m --output-format json); then
		return 1
	fi
	if [[ $(jq -r '.status' <<<"$response") != Accepted ]]; then
		echo "Apple notarization was not accepted; submission $(jq -r '.id // "unknown"' <<<"$response"). Retrieve the notarytool log." >&2
		return 1
	fi
}

notarize "$archive"
run xcrun stapler staple "$app" >/dev/null
run xcrun stapler validate "$app" >/dev/null
run spctl --assess --type execute --verbose=2 "$app" >/dev/null
ln -s /Applications "$stage/Applications"
dmg=$dist/Neptune-${version}-${platform}.dmg
run hdiutil create -volname Neptune -srcfolder "$stage" -format UDZO -ov "$dmg" >/dev/null
run codesign --timestamp --keychain "$keychain" --sign "$identity" "$dmg" >/dev/null
notarize "$dmg"
run xcrun stapler staple "$dmg" >/dev/null
run xcrun stapler validate "$dmg" >/dev/null
run spctl --assess --type open --context "context:primary-signature" --verbose=2 "$dmg" >/dev/null
