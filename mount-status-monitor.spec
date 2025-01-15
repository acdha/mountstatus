Name: mount-status-monitor
Version: 2.18.0
Release: 1%{?dist}
Summary: mount-status-monitor

License: CC0
URL: https://github.com/acdha/mountstatus/

Requires: bash

BuildRequires: systemd

BuildArch: x86_64

%description
mount-status-monitor packaged for EL8 systems

%prep

cp /mountstatus/target/release/mount_status_monitor %{_topdir}/BUILD/
cp /mountstatus/LICENSE %{_topdir}/BUILD/
cp /mountstatus/systemd/mount_status_monitor.service %{_topdir}/BUILD/

%install

mkdir -p %{buildroot}/%{_bindir}

install -m 0755 mount_status_monitor %{buildroot}%{_bindir}/%{name}
install -m 0644 mount_status_monitor.service %{_unitdir}/mount_status_monitor.service

%files
%license LICENSE
%{_bindir}/%{name}
