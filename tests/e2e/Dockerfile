FROM archlinux:latest
RUN pacman -Syu --noconfirm --needed git rust base-devel python python-pip pyside6 qt6-declarative qt6-tools \
    plasma-desktop plasma-workspace kwin kvantum qqc2-desktop-style kirigami breeze breeze-gtk spectacle dbus \
    xorg-xwayland imagemagick wofi kitty konsole sudo glib2 dconf procps-ng
RUN useradd -m -G wheel friend && echo 'friend ALL=(ALL) NOPASSWD: ALL' >/etc/sudoers.d/friend
