fn main() -> eframe::Result {
    unsafe { std::env::set_var("RUST_LOG", "sing_like_coding=debug") };
    env_logger::init();
    sing_like_coding::app::main()
}
