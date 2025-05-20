# 開発環境

## 初回

```
rustup component add rust-analyzer
rustup component add rust-src
cargo install cargo-watch --locked
```

## 毎回

```
cargo watch -x 'run -- --some-arg'
```

