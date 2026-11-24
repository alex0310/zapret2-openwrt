# AGENTS.md

## Обязательная проверка после изменений

После **каждого** изменения файлов в этом репозитории запускай проверку
Rust-workspace — даже если правились только shell-скрипты, Lua, JavaScript,
Makefile или документация:

```sh
cd rust
CARGO_TARGET_DIR=/tmp/opencode/cargo-check cargo check --all-targets
```

`CARGO_TARGET_DIR` вынесен за пределы репозитория: в `.gitignore` игнорируется
только `rust/target-*/`, а обычный `rust/target/` попал бы в untracked.

Изменение считается готовым только когда `cargo check --all-targets`
завершился без ошибок. Если проверка падает — сначала исправь причину.

## Контекст

- Rust-workspace: `rust/` (`cli`, `finder`). Toolchain зафиксирован в
  `rust/rust-toolchain.toml` (`nightly-2026-10-11`), `cargo` подхватывает его
  автоматически.
- Пакеты OpenWrt: `package/zapret2` и `package/luci-app-zapret2`.
- Установщик: `install-zapret2.sh` (скачивается с `main` через
  raw.githubusercontent.com), документация сборки: `BUILD.txt`.
