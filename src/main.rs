/*
    Paranoid mount monitor for POSIX operating systems

    The general idea is that some classes of storage failure require care to
    detect because any access to the mountpoint, including your monitoring code,
    will block and in the case of certain kernel bugs, that may either
    irrecoverable or until repeated TCP + NFS timeouts expire after multiple
    days. Asynchronous APIs could help except that e.g. the Linux async APIs
    don't include calls like stat(2).

    We try to avoid this situation by using an external child process with a
    timeout. If it fails to respond by the deadline, we'll send it a SIGKILL
    and avoid further checks until the process disappears.

    The major improvements of the Rust version compared to the older C version
    are the use of persistent state to avoid having more than one check pending
    for any given mountpoint and the ability to send metrics to a Prometheus
    push-gateway so they will be alertable even if the local system is severely
    degraded.
 */

extern crate libc;
extern crate hostname;
extern crate wait_timeout;
extern crate syslog;

#[macro_use]
extern crate cfg_if;

#[macro_use]
extern crate prometheus;
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::process;
use std::str;
use std::thread;
use std::time::{Duration, Instant};

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

mod get_mounts;

#[derive(Debug)]
struct MountStatus {
    last_checked: Instant,
    alive: bool,
    check_process: Option<process::Child>,
}

fn handle_syslog_error(err: std::io::Error) -> usize {
    // Convenience function allowing all of our syslog calls to use .unwrap_or_else
    eprintln!("Syslog failed: {}", err);
    0
}

fn main() {
    // TODO: command-line argument processing

    let poll_interval = Duration::from_secs(60);

    println!("mount_status_monitor checking mounts every {} seconds", poll_interval.as_secs());

    let syslog = syslog::unix(Facility::LOG_DAEMON)
        .unwrap_or_else(|err| {
            eprintln!("Unable to connect to syslog: {}", err);
            std::process::exit(1);
        });

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
            .map(|(_, status)| !status.alive)
            .filter(|&i| i)
            .count();

        syslog
            .info(format!(
                "Checked {} mounts; {} are dead",
                total_mounts,
                dead_mounts
            ))
            .unwrap_or_else(handle_syslog_error);

        // The Prometheus metrics are defined as floats so we need to convert;
        // for monitoring the precision loss in general is fine and it's
        // exceedingly unlikely to be relevant when counting the number of
        // mountpoints:
        TOTAL_MOUNTS.set(total_mounts as f64);
        DEAD_MOUNTS.set(dead_mounts as f64);

        if let Err(e) = prometheus::push_metrics(
            "mount_status_monitor",
            labels!{"instance".to_owned() => prometheus_instance.to_owned(), },
            "localhost:9091",
            prometheus::gather(),
        )
        {
            eprintln!("Unable to send pushgateway metrics: {}", e);
        }

        // Wait before checking again:
        thread::sleep(poll_interval);
    }
}


fn check_mounts(mount_statuses: &mut HashMap<String, MountStatus>, logger: &syslog::Logger) {
    let mount_points = get_mounts::get_mount_points();

    // FIXME: we need to purge stale entries which are no longer mounted

    for mount_point in mount_points {
        // Check whether there's a pending test:
        if let Some(mount_status) = mount_statuses.get_mut(&mount_point) {
            if mount_status.check_process.is_some() {
                let child = mount_status.check_process.as_mut().unwrap();

                match child.try_wait() {
                    Ok(Some(status)) => {
                        logger
                            .info(format!(
                                "Slow check for mount {} exited with {} after {} seconds",
                                mount_point,
                                status,
                                mount_status.last_checked.elapsed().as_secs()
                            ))
                            .unwrap_or_else(handle_syslog_error);
                        ()
                    }
                    Ok(None) => {
                        logger
                            .warning(format!(
                                "Slow check for mount {} has not exited after {} seconds",
                                mount_point,
                                mount_status.last_checked.elapsed().as_secs()
                            ))
                            .unwrap_or_else(handle_syslog_error);
                        continue;
                    }
                    Err(e) => {
                        logger.err(format!(
                            "Status update for hung check on mount {} returned an error after {} seconds: {}",
                            mount_point,
                            mount_status.last_checked.elapsed().as_secs(),
                            e
                        )).unwrap_or_else(handle_syslog_error);
                        ()
                    }
                }
            }
        }

        let mount_status = check_mount(&mount_point);

        if mount_status.alive {
            logger
                .debug(format!("Mount passed health-check: {}", mount_point))
                .unwrap_or_else(handle_syslog_error);
        } else {
            let msg = format!("Mount failed health-check: {}", mount_point);
            eprintln!("{}", msg);
            logger
                .err(msg)
                .unwrap_or_else(handle_syslog_error);
        }

        mount_statuses.insert(mount_point.to_owned(), mount_status);
    }
}

fn check_mount(mount_point: &str) -> MountStatus {
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
            /*
                The process has not exited and we're not going to wait for a
                potentially very long period of time for it to recover.

                We'll attempt to clean up the check process by killing it, which
                is defined as sending SIGKILL on Unix:

                https://doc.rust-lang.org/std/process/struct.Child.html#method.kill

                The mount_status structure returned will include this child
                process instance so future checks can perform a non-blocking
                test to see whether it has finally exited:
            */

            child.kill().unwrap_or_else(|err| {
                eprintln!("Unable to kill process {}: {:?}", child.id(), err)
            });

            mount_status.check_process = Some(child);
        }
        Some(exit_status) => {
            let rc = exit_status.code().unwrap();
            match rc {
                0 => mount_status.alive = true,
                _ => println!("Mount check failed with an unexpected return code: {:?}", rc),
            }
        }
    };

    mount_status
}
