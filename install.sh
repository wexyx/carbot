#!/usr/bin/env bash
set -euo pipefail
version=${CARBOT_VERSION:-latest}
prefix=${CARBOT_INSTALL_PREFIX:-$HOME/.local}
repo=${CARBOT_REPOSITORY:-wexyx/crabot}
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) target=aarch64-apple-darwin;;
  Darwin:x86_64) target=x86_64-apple-darwin;;
  Linux:x86_64) target=x86_64-unknown-linux-gnu;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu;;
  *) echo 'Unsupported platform. Use a supported macOS/Linux release or build from source.' >&2; exit 1;;
esac
[[ $prefix == /* && $prefix != / && $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || { echo 'Invalid install prefix or repository.' >&2; exit 1; }
[[ $version == latest || $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || { echo 'Version must be latest or vX.Y.Z.' >&2; exit 1; }
for cmd in curl tar; do command -v "$cmd" >/dev/null || { echo "Missing $cmd" >&2; exit 1; }; done
if ! command -v sha256sum >/dev/null && ! command -v shasum >/dev/null; then echo 'SHA-256 checker required.' >&2; exit 1; fi
base="https://github.com/$repo/releases"
if [[ $version == latest ]]; then
  resolved=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' "$base/latest")
  version=${resolved##*/}
  [[ $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || { echo 'No published release found.' >&2; exit 1; }
fi
archive="carbot-$target.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
for file in "$archive" "$archive.sha256"; do
  curl --proto '=https' --tlsv1.2 -fSL --retry 2 "$base/download/$version/$file" -o "$stage/$file" || { echo 'Release asset unavailable; no installation changes made.' >&2; exit 1; }
done
expected=$(awk 'NR==1 {print $1}' "$stage/$archive.sha256")
[[ $expected =~ ^[a-fA-F0-9]{64}$ ]] || { echo 'Invalid checksum file.' >&2; exit 1; }
if command -v sha256sum >/dev/null; then actual=$(sha256sum "$stage/$archive"); else actual=$(shasum -a 256 "$stage/$archive"); fi
[[ ${actual%% *} == "$expected" ]] || { echo 'Checksum mismatch; refusing installation.' >&2; exit 1; }
# Only extract the bundle produced by package-release.sh.
tar -tzf "$stage/$archive" | awk '$0 !~ /^carbot\// || $0 ~ /(^|\/)\.\.(\/|$)/ {bad=1} END {exit bad}' || { echo 'Unsafe archive paths.' >&2; exit 1; }
tar -tvzf "$stage/$archive" | awk 'substr($0,1,1)!="-" && substr($0,1,1)!="d" {bad=1} END {exit bad}' || { echo 'Archive links/devices are not allowed.' >&2; exit 1; }
tar -xzf "$stage/$archive" -C "$stage"
[[ -x $stage/carbot/bin/carbot && -x $stage/carbot/libexec/agent-node && -f $stage/carbot/web/index.html ]] || { echo 'Incomplete release bundle.' >&2; exit 1; }
mkdir -p "$prefix/share/carbot/releases" "$prefix/bin"
destination=$(mktemp -d "$prefix/share/carbot/releases/$version-$target.XXXXXX")
cp -R "$stage/carbot/." "$destination/"
if [[ -e $prefix/bin/carbot && ! -L $prefix/bin/carbot ]]; then echo "Refusing to overwrite $prefix/bin/carbot; bundle saved at $destination" >&2; exit 1; fi
ln -sfn "$destination/bin/carbot" "$prefix/bin/carbot"
printf 'Installed %s to %s\nRun: %s/bin/carbot\n' "$version" "$destination" "$prefix"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) printf 'Add this directory to PATH: %s/bin\n' "$prefix";; esac
echo 'No Rust/Node.js required. Linux command isolation needs bubblewrap; Python and vendor CLIs are optional separate dependencies.'
