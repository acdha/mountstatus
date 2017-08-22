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
use std::time::{Duration, Instant};
use std::thread;
use std::ptr;
use std::str;
use std::ffi;
use std::slice;
use std::collections::HashMap;

use libc::{c_int, kill, statfs};
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

#[derive(Debug)]
struct MountStatus {
    last_checked: Instant,
    alive: bool,
    process_id: Option<u32>, // TODO: decide whether this should be the actual Child process so we can poll it easily?
}

fn main() {
    // TODO: command-line argument processing
    let syslog = syslog::unix(Facility::LOG_DAEMON).unwrap();

    let prometheus_instance = hostname::get_hostname().unwrap();

    let mut mount_statuses = HashMap::<String, MountStatus>::new();

    for i in 0..5 {
        check_mounts(&mut mount_statuses, &syslog, &prometheus_instance);

        // Wait before checking again:
        thread::sleep(Duration::from_secs(i * 5));
    }
}

fn get_mount_points() -> Vec<String> {
    // FIXME: move this into a Darwin-specific module & implement the Linux version
    let mut raw_mounts_ptr: *mut statfs = ptr::null_mut();

    let rc = unsafe { getmntinfo(&mut raw_mounts_ptr, MNT_NOWAIT) };

    if rc < 0 {
        panic!("getmntinfo returned {:?}", rc);
    }

    let mounts = unsafe { slice::from_raw_parts(raw_mounts_ptr, rc as usize) };

    return mounts
        .iter()
        .map(|m| unsafe {
            ffi::CStr::from_ptr(&m.f_mntonname[0])
                .to_string_lossy()
                .into_owned()
        })
        .collect();
}

fn check_mounts(
    mount_statuses: &mut HashMap<String, MountStatus>,
    logger: &syslog::Logger,
    prometheus_instance: &String,
) {
    let mount_points = get_mount_points();

    // We calculate these values each time because a filesystem may have been
    // mounted or unmounted since the last check:
    let mut total_mounts = 0;
    let mut dead_mounts = 0;

    // FIXME: we need to purge stale entries which are no longer mounted

    for mount_point in mount_points {
        total_mounts += 1;

        // Check whether there's a pending test:
        match mount_statuses.get(&mount_point) {
            Some(mount_status) => {
                if mount_status.process_id.is_some() {
                    let pid = mount_status.process_id.unwrap();
                    let rc = unsafe { kill(pid as libc::pid_t, 0) };

                    if rc == 0 {
                        eprintln!(
                            "Skipping mount {} which has had a pending check (pid={}) for the last {} seconds",
                            mount_point,
                            pid,
                            mount_status.last_checked.elapsed().as_secs()
                        );
                        continue;
                    }
                }
            }
            None => {}
        }

        let mount_status = check_mount(&mount_point);

        if mount_status.alive {
            println!("Mount passed health-check: {}", mount_point);
            logger
                .debug(format!("Mount passed health-check: {}", mount_point))
                .unwrap();
        } else {
            // * If check fails, add to fail list and syslog
            eprintln!("Mount failed: {}", mount_point);
            logger
                .err(format!("Mount failed health-check: {}", mount_point))
                .unwrap();
            dead_mounts += 1;
        }

        mount_statuses.insert(mount_point.to_owned(), mount_status);
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
        labels!{"instance".to_owned() => prometheus_instance.to_owned(), },
        "localhost:9091",
        prometheus::gather(),
    ) {
        Err(e) => {
            eprintln!("Unable to send pushgateway metrics: {}", e);
        }
        _ => {}
    }
}

fn check_mount(mount_point: &str) -> MountStatus {
    // FIXME: decide how we're going to handle hung mounts – return the exit
    // status so it can be polled with try_wait?

    let mut cmd: Command;
    let mut mount_status = MountStatus {
        last_checked: Instant::now(),
        alive: false,
        process_id: None,
    };

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
            // The process has not exited yet:

            // We'll store a copy of the child's pid so it can be polled later,
            // possibly much later, to see whether it's finally exited:
            mount_status.process_id = Some(child.id());

            // We'll attempt to clean up by sending a SIGKILL but we won't wait
            // in case the kernel has blocked it in an uninterruptible state:
            child.kill().unwrap_or_else(|err| {
                eprintln!("Unable to kill process {}: {:?}", child.id(), err)
            });
        }
        Some(exit_status) => {
            let rc = exit_status.code().unwrap();
            match rc {
                0 => mount_status.alive = true,
                _ => println!("Unexpected response: {:?}", rc),
            }
        }
    };

    return mount_status;
}