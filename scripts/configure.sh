#!/usr/bin/env bash
# Point a fork of Leak at its own site address.
# Usage: scripts/configure.sh https://yourname.github.io/YourRepo/
set -euo pipefail

url="${1:?usage: scripts/configure.sh https://yourname.github.io/YourRepo/}"
url="${url%/}/"
host="$(echo "$url" | sed -E 's#https?://([^/]+)/.*#\1#')"
cd "$(dirname "$0")/.."

# Apps: Windows/Android shell and iOS
sed -i.bak -E "s#None => \"https?://[^\"]+\"#None => \"$url\"#" apps/shell/src-tauri/src/lib.rs
sed -i.bak -E "s#URL\(string: \"https?://[^\"]+\"\)#URL(string: \"$url\")#" apps/ios/Sources/Leak/LeakApp.swift
sed -i.bak -E "/WKAppBoundDomains/,/<\/array>/ s#<string>[^<]+</string>#<string>$host</string>#" apps/ios/Info.plist
rm -f apps/shell/src-tauri/src/lib.rs.bak apps/ios/Sources/Leak/LeakApp.swift.bak apps/ios/Info.plist.bak

echo "Apps now open $url"
echo "The website's own address is set automatically by the publish workflow."
