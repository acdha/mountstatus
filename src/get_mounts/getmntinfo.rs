// Wrapper for the BSD getmntinfo() API which returns a list of mountpoints
extern crate libc;

use std::ffi;
use std::ptr;
use std::slice;
use std::str;

use libc::{c_int, statfs};

pub static MNT_NOWAIT: i32 = 2;

extern "C" {
    #[cfg_attr(target_os = "macos", link_name = "getmntinfo$INODE64")]
    fn getmntinfo(mntbufp: *mut *mut statfs, flags: c_int) -> c_int;
}

pub fn get_mount_points() -> Vec<String> {
    // FIXME: move this into a Darwin-specific module & implement the Linux version
    let mut raw_mounts_ptr: *mut statfs = ptr::null_mut();

    let rc = unsafe { getmntinfo(&mut raw_mounts_ptr, MNT_NOWAIT) };

    if rc < 0 {
        panic!("getmntinfo returned {:?}", rc);
    }

    let mounts = unsafe { slice::from_raw_parts(raw_mounts_ptr, rc as usize) };

    mounts
        .iter()
        .map(|m| unsafe {
            ffi::CStr::from_ptr(&m.f_mntonname[0])
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}
