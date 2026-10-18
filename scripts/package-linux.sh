#!/usr/bin/env bash
# Package a native glibc Linux x64 build as DEB and AppImage.
# Usage: package-linux.sh --version V --binary PATH --appimagetool PATH
#                         --runtime PATH [--skip-deb]

set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$(dirname -- "${BASH_SOURCE[0]}")/package-common.sh"

version_info() {
	local version=$1
	local re='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-([0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?(\+([0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?$'
	if ! [[ $version =~ $re ]]; then
		echo "Invalid version: $version" >&2
		return 1
	fi
}

library_package() {
	local path=$1 candidate result
	# Ubuntu's merged /usr can expose /lib aliases through ldconfig/ldd while
	# dpkg records the /usr/lib path. Query both names before rejecting notices.
	for candidate in "$path" "$(readlink -f -- "$path")"; do
		if result=$(dpkg-query -S "$candidate" 2>/dev/null); then
			printf '%s\n' "${result%%:*}"
			return 0
		fi
		# A pacman host records the owning package under its own name.
		if result=$(pacman -Qoq "$candidate" 2>/dev/null); then
			printf '%s\n' "$result"
			return 0
		fi
	done
	echo "No installed package owns bundled library $path" >&2
	return 1
}

bundle_libraries() {
	local appdir=$1
	local library_dir=$appdir/usr/lib
	mkdir -p "$library_dir"
	# Platform/graphics drivers and glibc belong to the host, with the GBM,
	# Wayland and xkbcommon libraries those drivers and the keymap load: a
	# newer Mesa does not load against older Wayland copies, and an older
	# xkbcommon cannot read a newer system's keyboard layouts. Bundle the other
	# window libraries winit loads at runtime as well as linked transitive
	# dependencies.
	local exclude='^(ld-linux|lib(c|m|dl|rt|pthread|resolv|nss_[^.]+)\.so|lib(GL|EGL|GLX|GLdispatch|vulkan|drm|gbm|xkbcommon[a-z0-9-]*|wayland-[a-z]+))'
	local ldconfig_cache
	ldconfig_cache=$(/sbin/ldconfig -p)
	local names=(libX11.so.6 libXcursor.so.1 libXi.so.6 libXrandr.so.2 libXinerama.so.1)
	local browser=$appdir/usr/lib/neptune/browser
	local queue=("$appdir/usr/bin/neptune" "$browser/neptune-browser" "$browser/libcef.so")
	local name path
	for name in "${names[@]}"; do
		local escaped=${name//./\\.}
		path=$(grep -E "^[[:space:]]*${escaped} \(.*x86-64.*\) => " <<<"$ldconfig_cache" | head -n1 | awk '{print $NF}')
		if [[ -z $path ]]; then
			echo "Missing runtime library $name" >&2
			return 1
		fi
		queue+=("$path")
	done
	# NSS opens its own modules at runtime, where ldd cannot see them, and a
	# newer system's modules do not load into this older bundled NSS.
	local nss
	nss=$(grep -E '^[[:space:]]*libnss3\.so \(.*x86-64.*\) => ' <<<"$ldconfig_cache" | head -n1 | awk '{print $NF}')
	if [[ -z $nss ]]; then
		echo "Missing runtime library libnss3.so" >&2
		return 1
	fi
	local nss_directory
	nss_directory=$(dirname "$nss")
	for name in libsoftokn3 libfreeblpriv3 libfreebl3 libnssdbm3 libnssckbi; do
		local found=""
		local directory
		for directory in "$nss_directory/nss" "$nss_directory"; do
			if [[ -f $directory/$name.so ]]; then
				found=$directory/$name.so
				break
			fi
		done
		if [[ -z $found ]]; then
			case $name in
			libsoftokn3 | libfreeblpriv3)
				echo "Missing NSS module $name.so" >&2
				return 1
				;;
			*) continue ;;
			esac
		fi
		queue+=("$found")
		# FIPS mode verifies each module against the signature beside it.
		if [[ -f ${found%.so}.chk ]]; then
			install -m644 "${found%.so}.chk" "$library_dir/$name.chk"
		fi
	done
	declare -A seen
	local copyrights=$appdir/usr/share/doc/neptune/system-libraries
	mkdir -p "$copyrights"
	local owner dependencies dependency resolved dep_name dep_path
	while [[ ${#queue[@]} -gt 0 ]]; do
		path=${queue[-1]}
		queue=("${queue[@]:0:${#queue[@]}-1}")
		if [[ -n ${seen[$path]:-} ]]; then
			continue
		fi
		seen[$path]=1
		if [[ $(basename "$path") != neptune && "$path" != "$browser"/* ]]; then
			resolved=$(readlink -f -- "$path")
			cp -p -- "$resolved" "$library_dir/$(basename "$path")"
			owner=$(library_package "$path")
			if [[ -f /usr/share/doc/$owner/copyright ]]; then
				cp -p "/usr/share/doc/$owner/copyright" "$copyrights/$owner.copyright"
			elif compgen -G "/usr/share/licenses/$owner/*" >/dev/null; then
				# pacman keeps each package's licenses in their own directory.
				local directory=$copyrights/$owner.licenses
				if [[ ! -d $directory ]]; then
					mkdir "$directory"
					for dependency in /usr/share/licenses/$owner/*; do
						cp -p "$dependency" "$directory/$(basename "$dependency")"
					done
				fi
			elif pacman -Qi "$owner" >/dev/null 2>&1; then
				# Some pacman packages name their licenses in the database but
				# ship no license files; record that provenance as it is.
				{
					printf '%s\n' "License notice not installed by this host's package manager."
					grep -E '^(Version|Licenses)' < <(LC_ALL=C pacman -Qi "$owner") || true
				} >"$copyrights/$owner.copyright"
			else
				echo "Missing bundled library notice for $owner" >&2
				return 1
			fi
		fi
		dependencies=$(ldd "$path")
		if grep -q "not found" <<<"$dependencies"; then
			echo "Unresolved dependency in $(basename "$path")" >&2
			return 1
		fi
		while IFS= read -r dep_line; do
			dep_name=$(awk '{print $1}' <<<"$dep_line")
			dep_path=$(awk '{print $3}' <<<"$dep_line")
			if ! grep -qE "$exclude" <<<"$dep_name"; then
				queue+=("$dep_path")
			fi
		done < <(grep -E ' => /' <<<"$dependencies")
	done
}

usage() {
	echo "usage: package-linux.sh --version V --binary PATH --appimagetool PATH --runtime PATH [--skip-deb]" >&2
	exit 2
}

version=""
binary=""
appimagetool=""
runtime=""
skip_deb=false
while [[ $# -gt 0 ]]; do
	case $1 in
	--version)
		version=${2:?}
		shift 2
		;;
	--binary)
		binary=${2:?}
		shift 2
		;;
	--appimagetool)
		appimagetool=${2:?}
		shift 2
		;;
	--runtime)
		runtime=${2:?}
		shift 2
		;;
	--skip-deb)
		skip_deb=true
		shift
		;;
	*) usage ;;
	esac
done
[[ -n $version && -n $binary && -n $appimagetool && -n $runtime ]] || usage
version_info "$version"

dist=$REPO/dist
mkdir -p "$dist"
stage=$REPO/package/linux
# A stale stage would leak previous artifacts into this one.
rm -rf "$stage"
mkdir -p "$stage/usr/bin" "$stage/usr/lib"
install -m755 "$binary" "$stage/usr/bin/neptune"
browser_payload "$(dirname -- "$binary")" "$stage/usr/lib/neptune/browser" linux
desktop=$stage/usr/share/applications/rs.neptune.terminal.desktop
mkdir -p "$(dirname "$desktop")"
install -m644 "$REPO/packaging/neptune.desktop" "$desktop"
for size in 16 24 32 48 64 128 256 512; do
	icon=$stage/usr/share/icons/hicolor/${size}x${size}/apps/neptune.png
	mkdir -p "$(dirname "$icon")"
	install -m644 "$REPO/assets/icons/neptune-$size.png" "$icon"
done
notices "$stage/usr/share/doc/neptune"

if [[ $skip_deb == false ]]; then
	control=$stage/DEBIAN
	mkdir "$control"
	# Debian sorts ~ prereleases below the stable version; SemVer build metadata
	# is represented as a Debian revision without changing executable version.
	deb_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
	if [[ -n ${BASH_REMATCH[5]:-} ]]; then
		deb_version+="~${BASH_REMATCH[5]}"
	fi
	if [[ -n ${BASH_REMATCH[8]:-} ]]; then
		deb_version+="+${BASH_REMATCH[8]}"
	fi
	local_installed_size=$(find "$stage" -type f -printf '%s\n' | awk '{total += $1} END {printf "%d", total / 1024}')
	cat >"$control/control" <<EOF
Package: neptune
Version: $deb_version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Neptune maintainers <maintainers@neptune.rs>
Homepage: https://neptune.rs
Installed-Size: $local_installed_size
Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libx11-6, libxcursor1, libxi6, libxrandr2, libxinerama1, libegl1, libvulkan1, libnss3, libnspr4, libatk1.0-0, libatk-bridge2.0-0, libcups2, libxcomposite1, libxdamage1, libxfixes3, libgbm1, libpango-1.0-0, libcairo2, libasound2
Description: Native GPU terminal for focused work
 Independent PTY shells and persistent workspace organization.
EOF
	dpkg-deb --root-owner-group --build "$stage" "$dist/Neptune-${version}-linux-x64.deb"
	rm -rf "$control"
fi

# dpkg-deb installs this root-owned helper; AppImage uses user namespaces.
chmod 755 "$stage/usr/lib/neptune/browser/chrome-sandbox"
install -m644 "$desktop" "$stage/neptune.desktop"
install -m644 "$REPO/assets/icons/neptune-256.png" "$stage/neptune.png"
ln -s neptune.png "$stage/.DirIcon"
apprun=$stage/AppRun
# readlink -f resolves a symlink to AppRun, so an extracted AppDir can be
# launched through one; the AppImage runtime hands over its mount path.
cat >"$apprun" <<'EOF'
#!/bin/sh
APPDIR="$(CDPATH= cd -- "$(dirname -- "$(readlink -f -- "$0")")" && pwd)"
export LD_LIBRARY_PATH="$APPDIR/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$APPDIR/usr/bin/neptune" "$@"
EOF
chmod 755 "$apprun"
bundle_libraries "$stage"
# AppImage type2 runtime is MIT; include its pinned license with the payload.
install -m644 "$REPO/packaging/appimage-runtime-LICENSE" "$stage/usr/share/doc/neptune/AppImage-runtime-LICENSE"

ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$appimagetool" \
	--runtime-file "$runtime" \
	--mksquashfs-opt -processors --mksquashfs-opt 2 \
	"$stage" "$dist/Neptune-${version}-linux-x64.AppImage"
