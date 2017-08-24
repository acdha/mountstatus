cfg_if! {
        if #[cfg(target_os = "linux")] {
                mod getmntent;
                pub use self::getmntent::get_mount_points;
        } else {
                mod getmntinfo;
                pub use self::getmntinfo::get_mount_points;
        }
}
