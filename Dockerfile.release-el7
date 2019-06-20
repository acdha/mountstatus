FROM centos:7
ARG PACKAGE_VERSION
RUN if [ -z "${PACKAGE_VERSION}" ]; then echo "--build-arg PACKAGE_VERSION is required"; exit 1; fi

ENV RUST_ARCHIVE=rust-1.35.0-x86_64-unknown-linux-gnu.tar.gz
ENV RUST_DOWNLOAD_URL=https://static.rust-lang.org/dist/$RUST_ARCHIVE

RUN yum --quiet -y update && yum --quiet -y install git gcc curl openssl openssl-devel ca-certificates tar ruby-devel rubygems gcc make rpm-build libffi-devel && yum clean all --quiet

WORKDIR /rust

RUN curl -fsOSL $RUST_DOWNLOAD_URL \
    && curl -s $RUST_DOWNLOAD_URL.sha256 | sha256sum -c - \
    && tar -C /rust -xzf $RUST_ARCHIVE --strip-components=1 \
    && rm $RUST_ARCHIVE \
    && ./install.sh

RUN gem install --no-ri --no-rdoc json && gem install --no-ri --no-rdoc fpm && gem clean

WORKDIR /mountstatus

COPY Cargo.toml /mountstatus/
COPY src/ /mountstatus/src/
COPY packaging/systemd/ /mountstatus/systemd/
COPY packaging/sysconfig /mountstatus/

RUN cargo build --release && strip target/release/mount_status_monitor
RUN fpm -s dir -t rpm --rpm-dist el7 -n mount-status-monitor --version ${PACKAGE_VERSION} --replaces MountStatusMonitor target/release/mount_status_monitor=/usr/sbin/mount_status_monitor systemd/mount_status_monitor.service=/etc/systemd/system/mount_status_monitor.service

CMD /bin/cp -vr /mountstatus/*.rpm /host-packages-volume
