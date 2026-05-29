// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 启动桌面应用入口。
fn main() {
    debug_com_lib::run()
}
