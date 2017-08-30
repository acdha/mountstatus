// Wrapper for the BSD getmntinfo() API which returns a list of mountpoints
use std::ptr;
use std::slice;
use std::path::PathBuf;
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;

use libc::{c_int, statfs};

pub static MNT_NOWAIT: i32 = 2;

extern "C" {
    #[cfg_attr(target_os = "macos", link_name = "getmntinfo$INODE64")]
    fn getmntinfo(mntbufp: *mut *mut statfs, flags: c_int) -> c_int;
}

pub fn get_mount_points() -> Vec<PathBuf> {
    // FIXME: move this into a Darwin-specific module & implement the Linux version
    let mut raw_mounts_ptr: *mut statfs = ptr::null_mut();

    let rc = unsafe { getmntinfo(&mut raw_mounts_ptr, MNT_NOWAIT) };
    assert!(rc >= 0, "getmntinfo returned {:?}");
    assert!(!raw_mounts_ptr.is_null(), "getmntinfo failed to update list of mounts");

    let mounts = unsafe { slice::from_raw_parts(raw_mounts_ptr, rc as usize) };

    mounts
        .iter()
        .map(|m| unsafe {
            let bytes = CStr::from_ptr(&m.f_mntonname[0]).to_bytes();
            PathBuf::from(OsStr::from_bytes(bytes).to_owned())
        })
        .collect()
}
