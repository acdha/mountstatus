/*
    Paranoid mount monitor for most POSIX operating systems

    The general idea is that some classes of filesystem failure require care to
    detect because they block any process which attempts to access the
    mountpoint, including your monitoring code. Asynchronous APIs could help
    except that e.g. the Linux async APIs don't include calls like stat(2) and
    we'd like to avoid leaking a kernel request structure each time the monitor
    checks the mountpoints.

    We try to avoid this situation by using an external child process with a
    timeout so we can SIGKILL it and avoid further checks on that mountpoint
    until it terminates.

    The major improvements of this version of the program relative to the older
    C version are the use of persistent state to avoid having more than one
    check pending for any given mountpoint and the ability to send metrics to a
    Prometheus push-gateway so they will be alertable even if the local system
    is severely degraded.
 */

extern crate libc;
extern crate wait_timeout;
extern crate syslog;

#[macro_use]
extern crate prometheus;
#[macro_use]
extern crate lazy_static;

extern crate hostname;

use std::process::{Command, Stdio};
use std::time::Duration;
use std::ptr;
use std::str;
use std::ffi;
use std::slice;

use libc::{c_int, statfs};
use syslog::Facility;
use wait_timeout::ChildExt;

lazy_static! {
    static ref TOTAL_MOUNTS: prometheus::Gauge = register_gauge!(
        "total_mountpoints",
        "Total number of mountpoints"
    ).unwrap();

    static ref DEAD_MOUNTS: prometheus::Gauge = register_gauge!(
        "dead_mountpoints",
        "Number of unresponsive mountpoints"
    ).unwrap();
}

/*
target_os = linux android
target_os = macos freebsd dragonfly openbsd netbsd
*/

pub static MNT_NOWAIT: i32 = 2;

extern "C" {
    #[cfg_attr(target_os = "macos", link_name = "getmntinfo$INODE64")]
    fn getmntinfo(mntbufp: *mut *mut statfs, flags: c_int) -> c_int;
}

fn main() {
    // TODO: command-line argument processing
    let syslog = syslog::unix(Facility::LOG_DAEMON).unwrap();

    let prometheus_instance = hostname::get_hostname().unwrap();

    check_mounts(&syslog, prometheus_instance);
}

fn check_mounts(logger: &syslog::Logger, prometheus_instance: String) {
    let mut raw_mounts_ptr: *mut statfs = ptr::null_mut();

    let rc = unsafe { getmntinfo(&mut raw_mounts_ptr, MNT_NOWAIT) };

    if rc < 0 {
        logger
            .crit(format!("getmntinfo() returned an error: {}", rc))
            .unwrap();

        panic!("getmntinfo returned {:?}", rc);
    }

    let mounts = unsafe { slice::from_raw_parts(raw_mounts_ptr, rc as usize) };

    let mount_points = mounts.iter().map(|m| unsafe {
        ffi::CStr::from_ptr(&m.f_mntonname[0])
            .to_string_lossy()
            .into_owned()
    });

    let mut total_mounts = 0;
    let mut dead_mounts = 0;

    for mount_point in mount_points {
        total_mounts += 1;

        // for mount in mounts:
        // * Check whether there's a pending test
        // * If yes, skip
        // * If no, check mount
        if !check_mount(&mount_point) {
            // * If check fails, add to fail list and syslog
            eprintln!("Mount failed: {}", mount_point);
            logger
                .err(format!("Mount failed health-check: {}", mount_point))
                .unwrap();
            dead_mounts += 1;
        }
    }

    logger
        .info(format!(
            "Checked {} mounts; {} are dead",
            total_mounts,
            dead_mounts
        ))
        .unwrap();

    TOTAL_MOUNTS.set(total_mounts as f64);
    DEAD_MOUNTS.set(dead_mounts as f64);

    match prometheus::push_metrics(
        "mount_status_monitor",
        labels!{"instance".to_owned() => prometheus_instance,},
        "localhost:9091",
        prometheus::gather(),
    ) {
        Err(e) => {
            eprintln!("Unable to send pushgateway metrics: {}", e);
        }
        _ => {}
    }
}

fn check_mount(mount_point: &String) -> bool {
    // FIXME: decide how we're going to handle hung mounts – return the exit
    // status so it can be polled with try_wait?

    let mut cmd: Command;

    if mount_point == "/net" {
        // Simulate a hang for testing:
        cmd = Command::new("sleep");
        cmd.arg("10");
    } else {
        cmd = Command::new("/usr/bin/stat");
        cmd.arg(mount_point);
    }
    cmd.stdout(Stdio::null());

    let mut child = cmd.spawn().unwrap();

    match child.wait_timeout(Duration::from_secs(3)).unwrap() {
        None => {
            // The process is still running. We'll attempt to clean up by sending a
            // SIGKILL but we won't wait in case the kernel has blocked it in
            // an uninterruptible state:
            child.kill().unwrap()
        }
        Some(exit_status) => {
            let rc = exit_status.code().unwrap();
            match rc {
                0 => return true,
                _ => println!("Unexpected response: {:?}", rc),
            }
        }
    };

    return false;
}
