#!/usr/bin/env bash
# Shared packaging helpers: third-party notices, the browser payload and the
# pinned CEF runtime. Source this file, or run one subcommand:
#   package-common.sh notices DEST
#   package-common.sh browser-payload BINARY_DIR DEST PLATFORM

set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Resolve the pinned CEF distribution, rather than whatever libraries are on
# PATH, and print its directory. Build neptune-browser with the locked
# distribution first; CEF_PATH overrides the search for custom setups.
cef_runtime() {
	local binary_directory=$1 platform=$2
	local candidates=()
	local path
	if [[ -n ${CEF_PATH:-} ]]; then
		candidates+=("$CEF_PATH")
		for path in "$CEF_PATH"/154.0.34/cef_${platform}_*; do
			if [[ -e $path ]]; then
				candidates+=("$path")
			fi
		done
	fi
	for path in "$binary_directory"/build/cef-dll-sys-*/out/cef_${platform}_*; do
		if [[ -e $path ]]; then
			candidates+=("$path")
		fi
	done
	for path in "${candidates[@]}"; do
		if [[ -f $path/archive.json ]] && jq -e '.name | startswith("cef_binary_154.0.34+g14c5a08+")' "$path/archive.json" >/dev/null; then
			printf '%s\n' "$path"
			return 0
		fi
	done
	echo "Build neptune-browser with the locked CEF distribution before packaging" >&2
	return 1
}

# Assemble the browser host payload beside the application: the CEF runtime
# files it opens at runtime plus the built neptune-browser executable.
browser_payload() {
	local binary_directory=$1 destination=$2 platform=$3
	local runtime
	runtime=$(cef_runtime "$binary_directory" "$platform")
	if [[ -e $destination ]]; then
		echo "Browser payload destination already exists: $destination" >&2
		return 1
	fi
	mkdir -p "$destination"
	if [[ $platform == macos ]]; then
		cp -R "$runtime/Chromium Embedded Framework.framework" "$destination/Chromium Embedded Framework.framework"
		return 0
	fi
	local required=(icudtl.dat resources.pak chrome_100_percent.pak chrome_200_percent.pak v8_context_snapshot.bin CREDITS.html)
	if [[ $platform == linux ]]; then
		# The browser ELF must load CEF before libc for Chromium's close interposer.
		local linked cef_pos libc_pos
		linked=$(readelf -d "$binary_directory/neptune-browser")
		cef_pos=$(grep -boF '[libcef.so]' <<<"$linked" | head -n1 | cut -d: -f1)
		libc_pos=$(grep -boF '[libc.so.6]' <<<"$linked" | head -n1 | cut -d: -f1)
		if [[ -z $cef_pos || -z $libc_pos || $cef_pos -gt $libc_pos ]]; then
			echo "Browser ELF must load CEF before libc for Chromium's close interposer" >&2
			return 1
		fi
		required+=(libcef.so chrome-sandbox libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json)
		install -m755 "$binary_directory/neptune-browser" "$destination/neptune-browser"
	elif [[ $platform == windows ]]; then
		required+=(libcef.dll chrome_elf.dll bootstrapc.exe)
		install -m644 "$binary_directory/neptune_browser.dll" "$destination/neptune_browser.dll"
		local dll dll_name
		for dll in "$runtime"/*.dll; do
			dll_name=$(basename "$dll")
			case " ${required[*]} " in *" $dll_name "*) ;; *) required+=("$dll_name") ;; esac
		done
	else
		echo "Unknown browser platform" >&2
		return 1
	fi
	local file
	for file in "${required[@]}"; do
		install -m644 "$runtime/$file" "$destination/$file"
	done
	if [[ $platform == linux ]]; then
		# The minimal CEF archive still carries large, unmapped debug/symbol
		# sections. Keep the downloaded SDK intact, and ship only its runtime
		# code and dynamic exports in the private installer payload.
		strip --strip-unneeded "$destination/libcef.so"
		chmod 755 "$destination/libcef.so"
		chmod 4755 "$destination/chrome-sandbox"
	fi
	cp -R "$runtime/locales" "$destination/locales"
	install -m644 "$REPO/packaging/cef-LICENSE.txt" "$destination/CEF-LICENSE.txt"
}

# Third-party notices: the workspace's crates with their licenses, bundled
# theme provenance and every license file found beside each manifest.
notices() {
	local destination=$1
	mkdir -p "$destination"
	local output=$destination/THIRD-PARTY-NOTICES.txt
	local work=$output.tmp
	printf 'Bundled iTerm2-Color-Schemes: see themes/ for license, authors and provenance.\n\n' >"$work"
	local name version license manifest license_file root pattern file
	while IFS=$'\t' read -r name version license manifest license_file; do
		printf '%s %s\nLicense: %s\n\n' "$name" "$version" "$license" >>"$work"
		root=$(dirname "$manifest")
		local paths=()
		if [[ -n $license_file ]]; then
			paths+=("$root/$license_file")
		fi
		for pattern in LICENSE* LICENCE* COPYING* NOTICE* COPYRIGHT*; do
			if [[ -e $root/$pattern ]]; then
				paths+=("$root/$pattern")
			fi
		done
		if [[ ${#paths[@]} -gt 0 ]]; then
			local sorted
			sorted=$(printf '%s\n' "${paths[@]}" | sort -u)
			while IFS= read -r file; do
				if [[ -n $file && -f $file ]]; then
					printf '\n--- %s ---\n' "$(basename "$file")" >>"$work"
					cat "$file" >>"$work"
					printf '\n' >>"$work"
				fi
			done <<<"$sorted"
		fi
		printf '\n========================================================================\n\n' >>"$work"
	done < <(cargo metadata --locked --manifest-path "$REPO/Cargo.toml" --format-version 1 |
		jq -r '.packages | sort_by(.name, .version)[] | [.name, .version, (.license // "see license file"), .manifest_path, (.license_file // "")] | @tsv')
	mv "$work" "$output"
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
	case ${1:-} in
	notices)
		shift
		notices "${1:?usage: package-common.sh notices DEST}"
		;;
	browser-payload)
		shift
		browser_payload "${1:?usage: package-common.sh browser-payload BINARY_DIR DEST PLATFORM}" "${2:?.}" "${3:?.}"
		;;
	*)
		echo "usage: package-common.sh {notices DEST | browser-payload BINARY_DIR DEST PLATFORM}" >&2
		exit 2
		;;
	esac
fi
