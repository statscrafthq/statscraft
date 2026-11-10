#!/bin/sh
# Builds the Linux packages from target/release: a .deb, and a .tar.gz with the same files laid out
# under a prefix (bin/, share/) to unpack into ~/.local or /usr/local.
# From the repository root, after cargo build --release -p statscraft -p statscraft-gui:
#   packaging/linux/build.sh 0.1.0
# Needs dpkg-deb, objdump and GNU tar. Output: target/packages/.
set -eu
version=$1
out=target/packages
name=statscraft-$version-linux-x86_64
tree=$out/$name

rm -rf "$out"
install -Dm755 target/release/statscraft "$tree/bin/statscraft"
install -Dm755 target/release/statscraft-gui "$tree/bin/statscraft-gui"
install -Dm644 packaging/linux/statscraft.desktop "$tree/share/applications/statscraft.desktop"
install -Dm644 packaging/linux/statscraft.xml "$tree/share/mime/packages/statscraft.xml"
install -Dm644 crates/gui/icons/statscraft.svg "$tree/share/icons/hicolor/scalable/apps/statscraft.svg"
for size in 16 32 48 64 128 256; do
    install -Dm644 crates/gui/icons/statscraft-$size.png "$tree/share/icons/hicolor/${size}x$size/apps/statscraft.png"
done
install -Dm644 README.md "$tree/share/doc/statscraft/README.md"
install -Dm644 LICENSE "$tree/share/doc/statscraft/copyright"

tar -C "$out" --owner=0 --group=0 -czf "$out/$name.tar.gz" "$name"

# The .deb installs the same tree under /usr. shared-mime-info, desktop-file-utils and the icon
# theme refresh their caches through dpkg triggers, so the package needs no scripts.
deb=$out/deb
mkdir -p "$deb/DEBIAN"
# the binaries link only libc, libm and libgcc_s; the GUI libraries are loaded at run time
glibc=$(objdump -T "$tree"/bin/* | grep -o 'GLIBC_[0-9.]*' | cut -d_ -f2 | sort -uV | tail -n 1)
cp -a "$tree" "$deb/usr"
cat > "$deb/DEBIAN/control" <<EOF
Package: statscraft
Version: $version
Architecture: amd64
Maintainer: StatsCraft <https://github.com/statscrafthq/statscraft>
Installed-Size: $(du -sk "$deb/usr" | cut -f1)
Depends: libc6 (>= $glibc), libgcc-s1
Recommends: libgl1, libegl1, libxkbcommon-x11-0, libwayland-client0, xdg-desktop-portal
Section: science
Priority: optional
Homepage: https://github.com/statscrafthq/statscraft
Description: statistics workbench for data files and syntax
 StatsCraft reads and writes .sav and .zsav data files, runs syntax and
 prints tables as Markdown or JSON. The package holds the command line
 (statscraft) and the desktop window (statscraft-gui).
EOF
dpkg-deb --root-owner-group --build "$deb" "$out/statscraft_${version}_amd64.deb"
rm -rf "$deb" "$tree"
