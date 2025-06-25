# 開発環境

## 初回

```
rustup component add rust-analyzer
rustup component add rust-src
```

## 実行

`C-x p c`

## Debug

~/.emacs
```
(straight-use-package 'dap-mode)
(dap-mode 1)
(dap-ui-mode 1)
(require 'dap-lldb)
(setq dap-lldb-debug-program
      ;; VSCode で CodeLLDB をインストールしておく
      (list (expand-file-name "~/.vscode/extensions/vadimcn.vscode-lldb-1.11.5/adapter/codelldb.exe")))
(dap-register-debug-template
  "Rust::LLDB Run Configuration"
  (list :type "lldb-vscode"
        :request "launch"
        :name "Rust Debug"
        :program "f:/dev/sing_like_coding/target/debug/sing_like_coding.exe"
        :cwd "f:/dev/sing_like_coding"))
```

`M-x dap-debug`
