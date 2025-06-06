#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    unsafe { std::env::set_var("RUST_LOG", "sing_like_coding=debug") };
    env_logger::init();
    sing_like_coding::app::main().unwrap();
    Ok(())
}
