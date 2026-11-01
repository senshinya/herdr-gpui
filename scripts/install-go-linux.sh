#!/usr/bin/env bash
# The pinned build-time Go toolchain for the disposable Linux lint container.
set -euo pipefail
version=1.26.7
case "$(uname -m)" in
    x86_64) arch=amd64; sha=ffb5f8de10c62550dfddab66b36b57030721e0a44a3218e9e1181d7b59f121ca ;;
    aarch64) arch=arm64; sha=5a4ec883379d51ee9ce1040d5e87f8d35e20387574dd8c947feb01eabc3c1b37 ;;
    *) echo 'Unsupported Linux Go architecture' >&2; exit 1 ;;
esac
directory=${1:?Usage: install-go-linux.sh DESTINATION}
if [[ -x "$directory/go/bin/go" ]] && [[ $("$directory/go/bin/go" version) == "go version go$version linux/$arch" ]]; then exit 0; fi
mkdir -p "$directory"
archive=$(mktemp)
trap 'rm -f -- "$archive"' EXIT
curl -fsSL --proto '=https' --tlsv1.2 "https://go.dev/dl/go$version.linux-$arch.tar.gz" -o "$archive"
printf '%s  %s\n' "$sha" "$archive" | sha256sum --check --quiet
tar -xzf "$archive" -C "$directory"
