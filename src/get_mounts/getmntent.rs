// FIXME: work in progress Linux support wrapping getmntext() to match the BSD getmntinfo()

use libc::c_char;
use libc::c_int;
use libc::FILE;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
        unsafe { ::core::mem::zeroed() }
    }
}

extern "C" {
    fn getmntent_r(
        fp: *mut FILE,
        mntbuf: *mut mntent,
        buf: *mut c_char,
        buflen: c_int,
    ) -> *mut mntent;
}

pub fn get_mount_points() -> Vec<String> {
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
