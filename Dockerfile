FROM rust AS builder

WORKDIR /mountstatus

# We'll update the dependencies first so the build / compile stage for all
# of our dependencies can be cached:
# Until https://github.com/rust-lang/cargo/issues/1891 lands we'll use a fake main app
# to avoid clearing our cache except when dependencies actually change.
COPY Cargo.toml Cargo.lock /mountstatus/
RUN mkdir /mountstatus/src && echo "fn main() {}" > /mountstatus/src/main.rs && cargo build

# Now we'll build the actual project:
COPY src/ /mountstatus/src/
RUN cargo build

FROM debian:stretch

# FIXME: we can't test the binary without a local syslog daemon to connect to until we make this configurable:
RUN apt-get -qy update && apt-get -qy install inetutils-syslogd && apt-get clean
RUN echo "*.* /dev/console" > /etc/syslog.conf

COPY --from=builder /mountstatus/target/debug/mount_status_monitor /usr/local/sbin/

ENTRYPOINT service inetutils-syslogd start && timeout 8 mount_status_monitor --poll-interval=5
