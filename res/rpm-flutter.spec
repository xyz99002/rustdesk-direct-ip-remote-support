Name:       rustdesk-direct-ip-remote-support
Version:    1.4.9
Release:    0
Summary:    RPM package
License:    GPL-3.0
URL:        https://rustdesk.com
Vendor:     rustdesk <info@rustdesk.com>
Requires:   gtk3 libxcb libXfixes alsa-lib libva pam gstreamer1-plugins-base
Recommends: libayatana-appindicator-gtk3 libxdo
Provides:   libdesktop_drop_plugin.so()(64bit), libdesktop_multi_window_plugin.so()(64bit), libfile_selector_linux_plugin.so()(64bit), libflutter_custom_cursor_plugin.so()(64bit), libflutter_linux_gtk.so()(64bit), libscreen_retriever_plugin.so()(64bit), libtray_manager_plugin.so()(64bit), liburl_launcher_linux_plugin.so()(64bit), libwindow_manager_plugin.so()(64bit), libwindow_size_plugin.so()(64bit), libtexture_rgba_renderer_plugin.so()(64bit)

# https://docs.fedoraproject.org/en-US/packaging-guidelines/Scriptlets/

%description
The best open-source remote desktop client software, written in Rust.

%prep
# we have no source, so nothing here

%build
# we have no source, so nothing here

# %global __python %{__python3}

%install

mkdir -p "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support" && cp -r ${HBB}/flutter/build/linux/x64/release/bundle/* -t "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support"
mkdir -p "%{buildroot}/usr/bin"
# Pre-bake config.toml with the matching role (set ROLE=local or ROLE=remote before calling
# rpmbuild) so this package never requires a manual `rustdesk --setup-local`/`--setup-remote`
# step - mirrors the Windows Local/Remote MSI split. Falls back to bundling both as loose
# sample files if ROLE isn't set.
if [ "$ROLE" = "local" ] || [ "$ROLE" = "remote" ]; then
  install -Dm 644 $HBB/configs/$ROLE.toml "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/config.toml"
else
  install -Dm 644 $HBB/configs/local.toml "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/local.toml"
  install -Dm 644 $HBB/configs/remote.toml "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/remote.toml"
fi
install -Dm 644 $HBB/res/rustdesk.service -t "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/files"
install -Dm 644 $HBB/res/rustdesk.desktop -t "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/files"
install -Dm 644 $HBB/res/rustdesk-link.desktop -t "%{buildroot}/usr/share/rustdesk-direct-ip-remote-support/files"
install -Dm 644 $HBB/res/128x128@2x.png "%{buildroot}/usr/share/icons/hicolor/256x256/apps/rustdesk-direct-ip-remote-support.png"
install -Dm 644 $HBB/res/scalable.svg "%{buildroot}/usr/share/icons/hicolor/scalable/apps/rustdesk-direct-ip-remote-support.svg"

%files
/usr/share/rustdesk-direct-ip-remote-support/*
/usr/share/rustdesk-direct-ip-remote-support/files/rustdesk.service
/usr/share/icons/hicolor/256x256/apps/rustdesk-direct-ip-remote-support.png
/usr/share/icons/hicolor/scalable/apps/rustdesk-direct-ip-remote-support.svg
/usr/share/rustdesk-direct-ip-remote-support/files/rustdesk.desktop
/usr/share/rustdesk-direct-ip-remote-support/files/rustdesk-link.desktop

%changelog
# let's skip this for now

%pre
# can do something for centos7
case "$1" in
  1)
    # for install
  ;;
  2)
    # for upgrade
    systemctl stop rustdesk-direct-ip-remote-support || true
  ;;
esac

%post
cp /usr/share/rustdesk-direct-ip-remote-support/files/rustdesk.service /etc/systemd/system/rustdesk-direct-ip-remote-support.service
cp /usr/share/rustdesk-direct-ip-remote-support/files/rustdesk.desktop /usr/share/applications/rustdesk-direct-ip-remote-support.desktop
cp /usr/share/rustdesk-direct-ip-remote-support/files/rustdesk-link.desktop /usr/share/applications/rustdesk-direct-ip-remote-support-link.desktop
ln -sf /usr/share/rustdesk-direct-ip-remote-support/rustdesk /usr/bin/rustdesk
systemctl daemon-reload
# Fork config: only auto-enable/start the background service for a "remote" (inbound) install -
# config.toml is pre-baked per-role at build time (see %install above), so this is automatic;
# no manual `rustdesk --setup-local`/`--setup-remote` step is required in the normal case.
CONFIG_FILE=/usr/share/rustdesk-direct-ip-remote-support/config.toml
ROLE_INSTALLED=""
if [ -f "$CONFIG_FILE" ]; then
  ROLE_INSTALLED=$(grep -E '^[[:space:]]*role[[:space:]]*=' "$CONFIG_FILE" | head -1 | sed -E 's/^[[:space:]]*role[[:space:]]*=[[:space:]]*"([^"]*)".*/\1/')
fi
if [ "$ROLE_INSTALLED" = "remote" ]; then
  systemctl enable rustdesk-direct-ip-remote-support
  systemctl start rustdesk-direct-ip-remote-support
elif [ "$ROLE_INSTALLED" = "local" ]; then
  echo "rustdesk-direct-ip-remote-support: local (outbound-only) install - background service not enabled/started."
else
  echo "rustdesk-direct-ip-remote-support: config.toml missing or has no role - this should not happen on a normal install. Service installed but not enabled/started."
  echo "Run 'rustdesk --setup-remote' then 'systemctl enable --now rustdesk-direct-ip-remote-support' to accept inbound connections."
fi
update-desktop-database

%preun
case "$1" in
  0)
    # for uninstall
    systemctl stop rustdesk-direct-ip-remote-support || true
    systemctl disable rustdesk-direct-ip-remote-support || true
    rm /etc/systemd/system/rustdesk-direct-ip-remote-support.service || true
  ;;
  1)
    # for upgrade
  ;;
esac

%postun
case "$1" in
  0)
    # for uninstall
    rm /usr/bin/rustdesk || true
    rmdir /usr/lib/rustdesk-direct-ip-remote-support || true
    rmdir /usr/local/rustdesk-direct-ip-remote-support || true
    rmdir /usr/share/rustdesk-direct-ip-remote-support || true
    rm /usr/share/applications/rustdesk-direct-ip-remote-support.desktop || true
    rm /usr/share/applications/rustdesk-direct-ip-remote-support-link.desktop || true
    update-desktop-database
  ;;
  1)
    # for upgrade
    rmdir /usr/lib/rustdesk-direct-ip-remote-support || true
    rmdir /usr/local/rustdesk-direct-ip-remote-support || true
  ;;
esac
