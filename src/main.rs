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

use std::collections::HashMap;
use std::ffi;
use std::process;
use std::ptr;
use std::slice;
use std::str;
use std::thread;
use std::time::{Duration, Instant};

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

#[derive(Debug)]
struct MountStatus {
    last_checked: Instant,
    alive: bool,
    check_process: Option<process::Child>,
}

fn main() {
    // TODO: command-line argument processing

    let poll_interval = Duration::from_secs(60);

    let syslog = syslog::unix(Facility::LOG_DAEMON).unwrap();

    // FIXME: make Prometheus metric pushing optional
    let prometheus_instance = hostname::get_hostname().unwrap();

    let mut mount_statuses = HashMap::<String, MountStatus>::new();

    loop {
        check_mounts(&mut mount_statuses, &syslog);

        // We calculate these values each time because a filesystem may have been
        // mounted or unmounted since the last check:
        let total_mounts = mount_statuses.len();
        let dead_mounts = mount_statuses
            .iter()
            .map(|(_, status)| status.alive)
            .filter(|&i| i)
            .count();

        syslog
            .info(format!(
                "Checked {} mounts; {} are dead",
                total_mounts,
                dead_mounts
            ))
            .unwrap();

        // The Prometheus metrics are defined as floats so we need to convert;
        // for monitoring the precision loss in general is fine and it's
        // exceedingly unlikely to be relevant when counting the number of
        // mountpoints:
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

        // Wait before checking again:
        thread::sleep(poll_interval);
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

fn check_mounts(mount_statuses: &mut HashMap<String, MountStatus>, logger: &syslog::Logger) {
    let mount_points = get_mount_points();

    // FIXME: we need to purge stale entries which are no longer mounted

    for mount_point in mount_points {
        // Check whether there's a pending test:
        match mount_statuses.get_mut(&mount_point) {
            Some(mount_status) => if mount_status.check_process.is_some() {
                let child = mount_status.check_process.as_mut().unwrap();

                match child.try_wait() {
                    Ok(Some(status)) => {logger.info(format!(
                        "Slow check for mount {} exited with {} after {} seconds",
                        mount_point,
                        status,
                        mount_status.last_checked.elapsed().as_secs()
                    )).unwrap(); () },
                    Ok(None) => {
                            logger.err(format!(
                            "Slow check for mount {} has not exited after {} seconds",
                            mount_point,
                            mount_status.last_checked.elapsed().as_secs()
                        )).unwrap();
                        continue;
                    },
                    Err(e) => { logger.err(format!(
                        "Status update for hung check on mount {} returned an error after {} seconds: {}",
                        mount_point,
                        mount_status.last_checked.elapsed().as_secs(),
                        e
                    )).unwrap(); ()},
                }
            },
            None => {}
        }

        let mount_status = check_mount(&mount_point);

        if mount_status.alive {
            logger
                .debug(format!("Mount passed health-check: {}", mount_point))
                .unwrap();
        } else {
            // * If check fails, add to fail list and syslog
            eprintln!("Mount failed: {}", mount_point);
            logger
                .err(format!("Mount failed health-check: {}", mount_point))
                .unwrap();
        }

        mount_statuses.insert(mount_point.to_owned(), mount_status);
    }
}

fn check_mount(mount_point: &str) -> MountStatus {
    // FIXME: decide how we're going to handle hung mounts – return the exit
    // status so it can be polled with try_wait?

    let mut mount_status = MountStatus {
        last_checked: Instant::now(),
        alive: false,
        check_process: None,
    };

    let mut child = process::Command::new("/usr/bin/stat")
        .arg(mount_point)
        .stdout(process::Stdio::null())
        .spawn()
        .unwrap();

    match child.wait_timeout(Duration::from_secs(3)).unwrap() {
        None => {
            // The process has not exited yet:

            // We'll attempt to clean up by sending a SIGKILL but we won't wait
            // in case the kernel has blocked it in an uninterruptible state:
            child.kill().unwrap_or_else(|err| {
                eprintln!("Unable to kill process {}: {:?}", child.id(), err)
            });

            // We'll store a copy of the child so it can be polled later,
            // possibly much later, to see whether it's finally exited:
            mount_status.check_process = Some(child);
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
