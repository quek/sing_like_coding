use std::{env::current_exe, path::PathBuf};

pub fn dir_user_setting() -> PathBuf {
    let exe_path = current_exe().unwrap();
    let dir = exe_path.parent().unwrap();
    let dir = dir.join("user/setting");
    dir
}
