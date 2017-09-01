# Background

Operating systems have traditionally assumed that storage components are either
working or have failed and must be replaced. Most code has been developed using
the synchronous I/O model where operations wait until they either receive a
successful result or an error. This model is not suitable for complex storage
environments involving many devices or, especially, networks where failures are
more common and might be transient. Additionally, Linux, FreeBSD and macOS have
all had serious NFS client bugs and design flaws which significantly amplified
the damage from even a momentary failure or overload.

Unfortunately many programs have non-obvious triggers which cause them to scan
all mounted filesystems, causing them to hang as soon as any mounted filesystem
stops responding. In the case of NFS home directories the user experience is
especially bad because most desktop environments will completely hang trying to
access configuration files from the user's home directory, making the system
unusable.

Finally, very few applications have the complicated code required to report when a
storage request has blocked for a long period of time which makes it hard for a
system administrator to proactively correct the problem. In some cases a reboot
may be required but in many cases the fallout from a temporary outage can be
significantly reduced by using `umount -f` or, in certain situations on Linux,
`umount -f -l`, and remounting the filesystem so any new process will be
completely unaffected.

## What `mount_status_monitor` does

`mount_status_monitor` provides the missing notification solution for imperfect
storage. It's a simple daemon which periodically checks every mounted filesystem
using an asynchronous check with a timeout so it can report soft failures caused
by unresponsive storage as well as hard errors.

After each run is complete it will send a message to syslog:

    Checked 5 mounts; 0 are dead

Optionally, the [Prometheus push-gateway](https://prometheus.io/docs/instrumenting/pushing/)
will receive two metrics (`total_mountpoints` and `dead_mountpoints`) with the
same information for alerting and correlation purposes.

When a mount test fails the mountpoint will be sent to syslog and stderr:

    Mount failed health-check: /Volumes/TestSSHFS

There are several ways to simulate failures for testing. The easiest is to use a
user-mode filesystem such as sshfs, s3fs, etc. and use `kill -STOP` to freeze
the FUSE process long enough to trigger the unresponsive mount failure. For more
involved testing or if you are also evaluating system tuning options you can use
iptables to simulate packet loss or hard failure of an NFS server.

## Installation

Compiling the code requires a working [Rust](https://www.rust-lang.org) toolchain:

    cargo build --release

A Docker image is provided for testing basic functionality:

    docker build -t mountstatus . && docker run -it --rm mountstatus

### Running the monitor

For testing you can simply run `mount_status_monitor` directly and watch the
output. Note that while the process can run without elevated permissions it is
likely that this will generate error messages due to mountpoints which are
inaccessible.

In normal operation `mount_status_monitor` relies a supervisor such as Upstart,
systemd, or launchd to keep it running. See the `upstart` and `systemd`
directories for provided config files.

## Future Directions

A long term experiment is having `mount_status_monitor` actually attempt to run
`umount -f` (or `umount -f -l` on Linux) and remount the filesystem to reduce
the number of applications which hit a dead mountpoint. This may not be appropriate for all users and would require testing to avoid making the problem worse.
