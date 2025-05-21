fn main() -> eframe::Result {
    unsafe { std::env::set_var("RUST_LOG", "sawavi=debug") };
    env_logger::init();
    sawavi::app::main()
}
