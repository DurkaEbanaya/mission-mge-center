#!/bin/sh

set -eux

export SRC_PATH=${SRC_PATH:-"/hostfs"}

unset DISPLAY
unset WAYLAND_DISPLAY

ARCH="$(uname -m)"
SHARUN="https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/refs/heads/main/useful-tools/quick-sharun.sh"
EXTRA_PACKAGES="https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/refs/heads/main/useful-tools/get-debloated-pkgs.sh"

pacman -Syu --noconfirm base-devel zsync wget gtk4 libadwaita patchelf

wget "$SHARUN" -O ./quick-sharun
chmod +x ./quick-sharun

wget "$EXTRA_PACKAGES" -O ./get-debloated-pkgs
chmod +x ./get-debloated-pkgs

export VERSION=$(grep -oP 'version = \K.*' "$SRC_PATH/Cargo.toml" | head -n1 | tr -d '"')
export ICON=/usr/share/icons/hicolor/scalable/apps/io.missioncenter.MissionCenter.svg
export DESKTOP=/usr/share/applications/io.missioncenter.MissionCenter.desktop
export OUTPATH="$SRC_PATH/"
export OUTNAME=MissionCenter-"$VERSION"-"$ARCH".AppImage

./get-debloated-pkgs --add-common --prefer-nano

gtk_nocsd=
for candidate in \
    /usr/lib/missioncenter/libgtk-nocsd.so.0 \
    /usr/lib64/missioncenter/libgtk-nocsd.so.0 \
    /usr/lib/*/missioncenter/libgtk-nocsd.so.0; do
    if [ -r "$candidate" ]; then
        gtk_nocsd=$candidate
        break
    fi
done

if [ -z "$gtk_nocsd" ]; then
    echo "GTK-NoCSD library not found" >&2
    exit 1
fi

./quick-sharun \
    /usr/bin/missioncenter-kde \
    /usr/bin/missioncenter \
    /usr/bin/missioncenter-magpie \
    "$gtk_nocsd"
./quick-sharun --make-appimage
