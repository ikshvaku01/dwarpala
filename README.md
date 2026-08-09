# ◈ Dwarpala

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange.svg)](https://www.rust-lang.org)
[![Build](https://img.shields.io/badge/build-workspace-blue.svg)](#building-from-source)

**Compile and transform codebases into AI-ready datasets.**

Dwarpala scans a project directory, applies smart ignore rules (`.gitignore`-aware, plus per-language defaults for Node, Spring Boot, C++, and more), and compiles the remaining source files into a single structured document — ready to paste into an LLM context window.

It's a terminal UI built with [ratatui](https://ratatui.rs), with a live filesystem watcher that keeps the file list current as you edit.


---

## What this project actually is

Dwarpala is **not a commercial product**. It's a learning project with two goals:

1. Build a genuinely useful CLI/TUI tool for compiling codebases into AI-ready text.
2. Use it as a sandbox for learning how real-world "tiered" software is architected — offline license signing and verification with Ed25519, feature-gating, workspace-crate boundaries, and multi-repo/submodule structure.

**Everything here is open source (MIT).** The "Enterprise" tier isn't a paywall — there's no payment processor, no account system, and no actual restriction. It exists so the license-verification code has something real to gate, the way it would in a genuine commercial tool.

## Project structure

This repo uses a **git submodule** for `enterprise/`, mainly so the "Enterprise" learning module can be developed and versioned as a standalone unit, separate from the core tool.

```
dwarpala/ (this repo)
├── community/ Core CLI/TUI — the actual tool. MIT licensed.
├── enterprise/ → git submodule: dwarpala-enterprise
├── tools/
│ └── sign_license/ Ed25519 license-signing CLI (dev tool, not shipped)
├── Cargo.toml Workspace root
└── rust-toolchain.toml
```

Cloning the project requires initializing the submodule:

```bash
git clone --recurse-submodules https://github.com/your-org/dwarpala.git
```

If you already cloned without that flag:

```bash
git submodule update --init --recursive
```

> **Note:** `cargo build --workspace` will fail with a missing-manifest error if the submodule isn't initialized — `enterprise/` will just be an empty directory. This is the most common first-clone snag; the command above fixes it.

## Features

| | Community | "Enterprise" (learning module) |
|---|:---:|:---:|
| Scan, filter, and compile to one document | ✓ | ✓ |
| `.gitignore`-aware filtering | ✓ | ✓ |
| Project type auto-detection | ✓ | ✓ |
| Live filesystem watcher | ✓ | ✓ |
| Offline Ed25519 license verification | | ✓ |
| Feature-gating by license tier | | ✓ |

The Community edition (`community/`) is the actual working tool — build, filter, and file-compile logic all live here and are fully functional. The Enterprise crate wraps the same core and adds a license-activation dialog on top, purely to demonstrate how offline signed-license verification works end to end.

Some feature *names* in the tier table inside the app (Clipboard Copy, Analyzer, Masker, Test Generation, etc.) are placeholders for gating logic — the modules exist as stubs but aren't implemented yet. This is intentionally incremental as a learning exercise, not a hidden paywall.

## Installation

Build from source (no published crate yet):

```bash
git clone --recurse-submodules https://github.com/your-org/dwarpala.git
cd dwarpala
cargo build --release -p dwarpala
```

The binary will be at `target/release/dwarpala`.

## Usage

```bash
dwarpala <path-to-project>
```

Options:

--disable <feature,...> Disable specific features (e.g. DirStructure)
--enable <feature,...> Enable specific features


### Keyboard shortcuts

| Key | Action |
|---|---|
| `R` | Compile files into the output document |
| `M` | Open / close the menu |
| `Tab` | Cycle between Files / Patterns / Log panels |
| `↑` `↓` | Scroll the focused panel |
| `PgUp` `PgDn` | Scroll faster |
| `-` | Remove the selected ignore pattern |
| `Q` | Quit |

On first run, Dwarpala writes a default config to your OS config directory (e.g. `~/.config/dwarpala/config.json` on Linux) where you can customize output filename, path, and base ignore patterns.

## How it works

1. **Detect** — identifies project type (Node, Spring Boot, Maven, .NET, Rails, Python, C++) from marker files like `package.json` or `pom.xml`.
2. **Filter** — merges your base ignore list, `.gitignore` rules, and language-specific defaults (`community/assets/config/languages/*.json`) into one pattern set.
3. **Watch** — a debounced filesystem watcher keeps the included-files list live as you add, remove, or edit files.
4. **Compile** — on `[R]`, walks the filtered file list and writes a single output document, optionally prefixed with a directory-structure tree.

## Building from source

```bash
# Community binary only
cargo build --release -p dwarpala

# Enterprise binary (requires the submodule)
git submodule update --init --recursive
cargo build --release -p dwarpala-enterprise

# Everything, including the license-signing dev tool
cargo build --release --workspace
```

Cross-compiling for Windows and Linux is covered by the targets in `rust-toolchain.toml`:

```bash
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
cargo build --release --target x86_64-unknown-linux-gnu -p dwarpala
cargo build --release --target x86_64-pc-windows-gnu -p dwarpala
```

## The "Enterprise" learning module

`enterprise/` (submodule → [`dwarpala-enterprise`](https://github.com/your-org/dwarpala-enterprise)) demonstrates a full offline license-verification flow:

- `DW.<header>.<claims>.<signature>` tokens, Ed25519-signed
- Obfuscated public-key storage with key-rotation support via `kid`
- A `.license` file format with human-readable display metadata
- `tools/sign_license` — a companion CLI to generate keypairs and issue signed licenses for testing

See `enterprise/README.md` for the key-rotation procedure and how to issue yourself a test license.

## Contributing

This is a learning-in-public project — contributions, questions, and "why did you do it this way" issues are all welcome, especially around the Community tool itself.

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
