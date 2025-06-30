all:
	cargo build --workspace && set RUST_BACKTRACE=1 && cargo run -p sing_like_coding

release:
	cargo build --release --workspace && set RUST_BACKTRACE=1 && cargo run -p sing_like_coding
