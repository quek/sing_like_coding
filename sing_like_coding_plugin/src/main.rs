fn main() {
    unsafe { std::env::set_var("RUST_LOG", "sing_like_coding_plugin=debug") };
    env_logger::init();
    log::debug!("Start sing like coding plugin...");
    sing_like_coding_plugin::app::main();
}
