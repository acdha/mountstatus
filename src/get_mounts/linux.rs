// Wrapper for the Linux getmntent() API which returns a list of mountpoints

use std::mem;

use std::path::PathBuf;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::ffi::CStr;

use libc::c_char;
use libc::c_int;
use libc::FILE;

#[repr(C)]
#[derive(Debug)]
struct mntent {
    mnt_fsname: *mut c_char,
    mnt_dir: *mut c_char,
    mnt_type: *mut c_char,
    mnt_opts: *mut c_char,
    mnt_freq: c_int,
    mnt_passno: c_int,
}

impl Default for mntent {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

extern "C" {
    fn getmntent(fp: *mut FILE) -> *mut mntent;
    fn setmntent(filename: *const c_char, _type: *const c_char) -> *mut FILE;
    fn endmntent(fp: *mut FILE) -> c_int;
}

pub fn get_mount_points() -> Vec<PathBuf> {
    let mut mount_points: Vec<PathBuf> = Vec::new();

    // The Linux API is somewhat baroque: rather than exposing the kernel's view of the world
    // you are expected to provide it with a mounts file which traditionally might have been
    // something like /etc/mtab but should be /proc/self/mounts (n.b. /proc/mounts is just a
    // symlink to /proc/self/mounts).
    let mount_filename = "/proc/self/mounts\0";
    let flags = "r\0";

    let mount_file_handle = unsafe { setmntent(mount_filename.as_ptr() as *const _, flags.as_ptr() as *const _) };

    if mount_file_handle.is_null() {
        panic!(
            "Attempting to read mounts from {} failed!",
            &mount_filename[..mount_filename.len() - 1]
        );
    }

    loop {
        let mount_entry = unsafe { getmntent(mount_file_handle) };
        if mount_entry.is_null() {
            break;
        } 

        let bytes = unsafe {
            CStr::from_ptr((*mount_entry).mnt_dir).to_bytes()
        };
        let mount_point = PathBuf::from(OsStr::from_bytes(bytes).to_owned());
        mount_points.push(mount_point);
    }

    let rc = unsafe { endmntent(mount_file_handle) };
    assert!(rc == 1, "endmntent() is always supposed to return 1 but returned {}", rc);

    mount_points
}
