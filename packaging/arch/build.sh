#!/usr/bin/env bash
set -euo pipefail

root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
cd "$root"
if [[ -n $(git status --porcelain --untracked-files=normal) ]]; then
  echo 'Review and commit source changes before creating an Arch source archive.' >&2
  exit 1
fi
version=$(cargo metadata --locked --no-deps --format-version 1 | node -e '
  const data = JSON.parse(require("node:fs").readFileSync(0, "utf8"));
  console.log(data.packages.find((pkg) => pkg.name === "merframe").version);
')
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Arch packaging requires a stable version.' >&2; exit 1; }
archive="packaging/arch/Merframe-$version.tar.gz"
git archive --format=tar.gz --prefix="Merframe-$version/" -o "$archive" HEAD
export MERFRAME_SOURCE_SHA256
MERFRAME_SOURCE_SHA256=$(sha256sum "$archive" | cut -d ' ' -f 1)
export MERFRAME_PACKAGE_VERSION="$version"
cd packaging/arch
makepkg "$@"
