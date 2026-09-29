#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
target=${1:?usage: package-release.sh TARGET [BINARY]}
[[ $target =~ ^[a-zA-Z0-9_-]+$ ]] || exit 2
binary=${2:-target/release/agent-node}
[[ -x $binary && -f apps/web/dist/index.html ]] || { echo 'Build Rust and Web first.' >&2; exit 1; }
if [[ -n ${CARBOT_RELEASE_VERSION:-} ]]; then
  expected="Carbot v${CARBOT_RELEASE_VERSION#v}"
  actual=$("$binary" --version)
  [[ $actual == "$expected" ]] || { echo "Version mismatch: expected $expected, got $actual. Rebuild with CARBOT_RELEASE_VERSION." >&2; exit 1; }
fi
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/carbot/bin" "$stage/carbot/libexec" dist
cp "$binary" "$stage/carbot/libexec/agent-node"
cp install.sh "$stage/carbot/libexec/install.sh"
cp scripts/release/carbot "$stage/carbot/bin/carbot"
chmod 755 "$stage/carbot/bin/carbot"
cp -R apps/web/dist "$stage/carbot/web"
cp README.md "$stage/carbot/README.md"
cp LICENSE "$stage/carbot/LICENSE"
cp CONTRIBUTING.md "$stage/carbot/CONTRIBUTING.md"
cp -R docs "$stage/carbot/docs"
mkdir -p "$stage/carbot/skills"
tar --exclude=.runtime --exclude=node_modules -C skills -cf - system | tar -C "$stage/carbot/skills" -xf -
archive="carbot-$target.tar.gz"
tar -czf "dist/$archive" -C "$stage" carbot
cd dist
if command -v sha256sum >/dev/null; then sha256sum "$archive" > "$archive.sha256"; else shasum -a 256 "$archive" > "$archive.sha256"; fi
printf 'Created dist/%s\n' "$archive"
