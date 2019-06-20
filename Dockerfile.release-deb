FROM rust:1.35
ARG PACKAGE_VERSION
RUN if [ -z "${PACKAGE_VERSION}" ]; then echo "--build-arg PACKAGE_VERSION is required"; exit 1; fi

RUN apt-get -qqy update && apt-get -qqy install ruby ruby-dev rubygems build-essential && apt-get clean
RUN gem install --no-ri --no-rdoc --quiet fpm && gem clean

WORKDIR /mountstatus

COPY Cargo.toml Cargo.lock /mountstatus/
COPY src/ /mountstatus/src/
RUN cargo build --release

WORKDIR /package-build

RUN mv /mountstatus/target/release/mount_status_monitor /package-build/
COPY packaging/sysconfig /package-build/
COPY packaging/systemd/mount_status_monitor.service /package-build/

# This is good for a ~60% size reduction:
RUN strip mount_status_monitor

RUN fpm -s dir -t deb -n mount-status-monitor --version ${PACKAGE_VERSION} --replaces MountStatusMonitor --config-files /etc/sysconfig/mount_status_monitor mount_status_monitor=/usr/sbin/mount_status_monitor mount_status_monitor.service=/etc/systemd/system/mount_status_monitor.service sysconfig=/etc/sysconfig/mount_status_monitor

CMD /bin/cp -vr /package-build/*.deb /host-packages-volume
