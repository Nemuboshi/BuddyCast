Please write the code using Rust 2024 Edition.

Code requirements:

1. Do not use `unsafe`.
2. Do not use `unwrap` or `expect` in business logic. They are allowed only in test code.
3. Keep `main.rs` as a thin startup entry point only. Put the core logic in `lib.rs` and separate modules.
4. Use `anyhow` for application-level errors and `thiserror` for domain-specific errors.
5. Whenever a new feature is added, add corresponding tests at the same time.
6. The code must pass all of the following commands:
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
7. Generate detailed English comments throughout the project.
8. Prefer using Caveman Skills and the context_mode MCP whenever they are applicable.