#!/bin/sh
# Builds the desktop app for macOS as an app bundle and packs it in a disk
# image: one universal app (Apple silicon and Intel) carrying the data it
# reads (the actor, background and tile catalogs). NCPatcher is not in it: it
# is a tool of its own that the app finds on the PATH, or where Preferences
# says. CI runs it on every packaging run (see .github/workflows/package.yml).
#
#   packaging/macos/build.sh [work-dir]
#
# Run it on a Mac with the Xcode command line tools and Rust's two macOS
# targets (`rustup target add aarch64-apple-darwin x86_64-apple-darwin`).
# Everything is written under work-dir, by default build-macos/ in the source
# tree: the app bundle and the disk image land there.
#
# How the app is signed depends on two variables:
#
#   MACOS_SIGN_IDENTITY   a Developer ID Application identity in the keychain,
#                         as `security find-identity -v -p codesigning` lists
#                         it. The app and the disk image are signed with it.
#   MACOS_NOTARY_PROFILE  credentials stored with `xcrun notarytool
#                         store-credentials`. The disk image is sent to Apple
#                         to be notarized and the ticket stapled to it; this
#                         takes a few minutes. MACOS_NOTARY_KEYCHAIN names the
#                         keychain they are in when it is not the login one.
#
# With neither, the app is signed ad hoc and not notarized, so the first time
# it is opened macOS asks for it to be allowed in System Settings > Privacy &
# Security.
set -eu

[ "$(uname -s)" = Darwin ] || { echo "the macOS app is built on macOS" >&2; exit 1; }

identity=${MACOS_SIGN_IDENTITY:-}
notary_profile=${MACOS_NOTARY_PROFILE:-}
[ -z "$notary_profile" ] || [ -n "$identity" ] || {
    echo "MACOS_NOTARY_PROFILE needs MACOS_SIGN_IDENTITY: Apple notarizes only signed apps" >&2
    exit 1
}

root=$(cd "$(dirname "$0")/../.." && pwd)
work=${1:-$root/build-macos}
mkdir -p "$work"
work=$(cd "$work" && pwd)
app="$work/Big Star Editor.app"

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || { echo "no version in Cargo.toml" >&2; exit 1; }

# The oldest macOS the app runs on, for the compiler and for Info.plist alike.
export MACOSX_DEPLOYMENT_TARGET=11.0
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    (cd "$root" && cargo build --release --locked -p bigstar --target "$target")
done

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
target_dir=${CARGO_TARGET_DIR:-$root/target}
lipo -create -output "$app/Contents/MacOS/bigstar" \
    "$target_dir/aarch64-apple-darwin/release/bigstar" \
    "$target_dir/x86_64-apple-darwin/release/bigstar"
# The program is most of its size in symbols.
strip "$app/Contents/MacOS/bigstar"
# Where default_data_dir (crates/host) looks for them: ../Resources/data from
# the executable.
cp -r "$root/data" "$app/Contents/Resources/data"
cp "$root/LICENSE" "$root/docs/licenses.md" "$app/Contents/Resources/"

# Finder's icon, in every size it asks for that the 512-pixel PNG can give.
iconset=$work/bigstar.iconset
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$root/packaging/bigstar.png" --out "$iconset/icon_${size}x${size}.png" > /dev/null
    [ "$size" = 512 ] || sips -z $((size * 2)) $((size * 2)) "$root/packaging/bigstar.png" \
        --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns -o "$app/Contents/Resources/bigstar.icns" "$iconset"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>Big Star Editor</string>
	<key>CFBundleExecutable</key>
	<string>bigstar</string>
	<key>CFBundleIconFile</key>
	<string>bigstar</string>
	<key>CFBundleIdentifier</key>
	<string>io.github.ndymario.bigstar</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Big Star Editor</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>$MACOSX_DEPLOYMENT_TARGET</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
EOF

# Without a signature over the whole bundle, macOS calls a downloaded app
# damaged rather than offering to open it. Notarizing also asks for the
# hardened runtime and a secure timestamp, which an ad hoc signature cannot
# have.
if [ -n "$identity" ]; then
    codesign --force --sign "$identity" --options runtime --timestamp "$app"
else
    codesign --force --sign - "$app"
fi

# The disk image holds the app beside a link to Applications, to drag it onto.
stage=$work/dmg
rm -rf "$stage"
mkdir -p "$stage"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
out=$work/BigStarEditor-$version-macOS.dmg
rm -f "$out"
hdiutil create -volname "Big Star Editor" -srcfolder "$stage" -format UDZO -o "$out" > /dev/null
rm -rf "$stage" "$iconset"
[ -z "$identity" ] || codesign --force --sign "$identity" --timestamp "$out"

# Notarizing the disk image notarizes the app in it too. The ticket is
# stapled to the image, so it opens without asking Apple; the app copied out
# of it is looked up online the first time it runs.
if [ -n "$notary_profile" ]; then
    result=$(xcrun notarytool submit "$out" --keychain-profile "$notary_profile" \
        ${MACOS_NOTARY_KEYCHAIN:+--keychain "$MACOS_NOTARY_KEYCHAIN"} \
        --wait --output-format json)
    id=$(printf '%s' "$result" | plutil -extract id raw -o - -)
    status=$(printf '%s' "$result" | plutil -extract status raw -o - -)
    if [ "$status" != Accepted ]; then
        echo "notarization $id ended $status:" >&2
        xcrun notarytool log "$id" --keychain-profile "$notary_profile" \
            ${MACOS_NOTARY_KEYCHAIN:+--keychain "$MACOS_NOTARY_KEYCHAIN"} >&2 || true
        exit 1
    fi
    xcrun stapler staple "$out"
fi

echo "Created $out"
