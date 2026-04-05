# oh-my-code

A Rust-native interactive terminal coding assistant. Rebuilds the Claude Code experience in Rust with a pluggable provider layer, a native search toolchain, and a concurrent tool executor with read/write safety partitioning.

## Features

- **Multi-provider**: Claude (Anthropic Messages API), OpenAI / GPT, Zhipu, and MiniMax via an OpenAI-compatible adapter. Add a new provider by extending `create_provider` in `src/model/mod.rs`.
- **Streaming agent loop**: Assistant text streams to the terminal as it arrives; tool calls are dispatched and fed back into the conversation until the model stops calling tools.
- **11 built-in tools**: `think`, `grep`, `glob`, `file_read`, `file_edit`, `file_write`, `bash`, `enter_plan_mode`, `exit_plan_mode`, `web_fetch`, `web_search`.
- **Concurrent tool execution with safety partitioning**: Read-only tools run in parallel via `join_all`; write tools run sequentially in arrival order.
- **Plan / Act modes**: An atomic shared flag gates writes. In Plan mode, the model can read and reason but cannot mutate files or run shell commands.
- **Native search toolchain**: Uses `grep-searcher` + `grep-regex` + `ignore` + `syntect` directly — no shelling out to `rg` or `fd`.
- **Interactive REPL**: `rustyline`-based line editor with slash commands (`/help`, `/model`, `/session`, `/clear`, `/quit`).
- **File-based sessions**: Conversations are persisted as JSON under `~/.config/oh-my-code/sessions/`.

## Install

Requires a recent stable Rust toolchain (install via [rustup](https://rustup.rs)).

```bash
git clone <this-repo>
cd oh-my-code
cargo build --release
```

The binary lands at `target/release/oh-my-code`.

## Configuration

On first run, a default config is written to `~/.config/oh-my-code/config.toml`. Edit it to change the default provider, model, search ignore patterns, or session storage directory. The in-repo template lives at `config/default.toml`.

Each provider reads its API key from an environment variable:

| Provider  | Env var             |
|-----------|---------------------|
| `claude`  | `ANTHROPIC_API_KEY` |
| `openai`  | `OPENAI_API_KEY`    |
| `zhipu`   | `ZHIPU_API_KEY`     |
| `minimax` | `MINIMAX_API_KEY`   |

## Usage

```bash
ANTHROPIC_API_KEY=<key> ./target/release/oh-my-code
```

Once in the REPL, type a request in natural language. Useful slash commands:

- `/help` — list commands
- `/model` — switch model
- `/session` — list / load / save sessions
- `/clear` — clear the current conversation history
- `/quit` — exit

## Development

```bash
cargo test              # full test suite
cargo test <path>       # run a subset, e.g. cargo test model::types
cargo clippy --all-targets
cargo fmt
```

Note: `oh-my-code` is a binary-only crate, so `cargo test --lib ...` will fail — use `cargo test <module_path>` instead.

See [`CLAUDE.md`](CLAUDE.md) for a deeper architectural tour.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE) for the full text.
