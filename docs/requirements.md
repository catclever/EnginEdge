# EnginEdge: Product Requirements & Technical Specification

> **Positioning**: Ultra-fast, single-binary edge executor and Model Context Protocol (MCP) server for the Descartes & Dalembert ecosystem.
> **Architecture Pattern**: *"Soul in Cloud, Body at Edge"* — Centralized cognitive intelligence in the cloud, secure native AST manipulation & tool execution at the edge.

---

## 1. Executive Summary & Vision

Modern AI coding assistants (such as Cursor, Claude Desktop, Antigravity, and Zed) rely heavily on accurate codebase context and safe local tool execution. However, traditional approaches suffer from two major flaws:
1. **Fragile Line-Number Editing**: Relying on line numbers (`start_line`, `end_line`) causes frequent off-by-one errors and line-drift hallucinations whenever code changes.
2. **Heavyweight Local Runtimes**: Running massive Python or Node.js agent runtimes locally consumes substantial RAM and battery, cluttering developer environments.

**EnginEdge** solves these problems by providing a native, lightweight (pure Rust), zero-runtime CLI and MCP server that:
- Acts as a standard **Model Context Protocol (MCP) server** over `stdio`, immediately compatible with Cursor, Claude Desktop, and Antigravity.
- Integrates native **Tree-sitter** grammar parsers to navigate, inspect, and replace code at the **Abstract Syntax Tree (AST) symbol level** without line-number drift.
- Connects transparently via **WebSocket** to the cloud-side **Descartes Produit** and **Dalembert** intelligence engines to fetch open-source blueprints, documentation, and living test specs on demand.
- Enforces an **Anti-Leak Security Jail** to ensure local secrets (`.env`, `id_rsa`, AWS tokens) are strictly blocked from cloud exfiltration.

---

## 2. System Architecture

```text
+-------------------------------------------------------------------------+
|                  Developer Desktop Host (IDE / CLI)                     |
|                                                                         |
|  [Cursor / Claude Desktop / Zed]      [CI/CD / Terminal Automation]     |
+-------------------------------------------------------------------------+
                         │ (stdio JSON-RPC)                  │ (CLI args)
                         ▼                                   ▼
+-------------------------------------------------------------------------+
|                      EnginEdge (Rust Native Binary)                     |
|                                                                         |
|  +-------------------------------------------------------------------+  |
|  | 1. MCP Server Engine (JSON-RPC 2.0 over stdio)                    |  |
|  +-------------------------------------------------------------------+  |
|                                  │                                      |
|  +-------------------------------------------------------------------+  |
|  | 2. Security Jail (Workspace Chroot, Path Scoping, Secret Filter)  |  |
|  +-------------------------------------------------------------------+  |
|         │                                                 │             |
|         ▼                                                 ▼             |
|  +---------------------------+       +-------------------------------+  |
|  | 3. Tree-sitter AST Engine |       | 4. Cloud WebSocket Bridge     |  |
|  | - get_outline             |       | - Handshake with Hub          |  |
|  | - read_symbol             |       | - Dynamic Tool Proxying       |  |
|  | - replace_symbol          |       | - Knowledge Vault query       |  |
|  +---------------------------+       +-------------------------------+  |
+-------------------------------------------------------------------------+
                                                     ▲
                                                     │ WebSocket (TLS / JSON)
                                                     ▼
+-------------------------------------------------------------------------+
|                    Cloud Intelligence Layer (Ruby)                      |
|                                                                         |
|  - Descartes Produit: Distributed connection hub & session state        |
|  - Dalembert: GitHub repo survey, code search, and doc extraction       |
|  - Descartes Knowledge Vault: Persistent architectural memory           |
+-------------------------------------------------------------------------+
```

---

## 3. Core Capability Modules

### Module A: MCP Server & Host Interface
- **Communication Protocol**: Model Context Protocol (MCP) spec 2024-11-05 via standard I/O (`stdin` / `stdout`).
- **Supported Hosts**: Cursor, Claude Desktop, Antigravity, Zed, Windsurf.
- **Headless Mode**: Supports CLI execution (e.g. `enginedge run --task "..."`) for non-interactive scripts.

### Module B: Universal Tree-sitter AST Engine
Multi-language parsing with sub-millisecond AST queries:
- **Supported Languages (Phase 1)**: Rust, Go, TypeScript/JavaScript, Python, Ruby.
- **Tools**:
  - `get_outline(file_path)`: Returns classes, structs, functions, interfaces, and signatures without dumping entire source bodies into context.
  - `read_symbol(file_path, symbol_path)`: Extracts the exact source code for a targeted symbol (e.g. `Server::handle_message` or `auth.verify_token`).
  - `replace_symbol(file_path, symbol_path, new_code)`: Atomically substitutes the AST node with formatted new code, validating syntax before writing to disk.

### Module C: Distributed Cloud Intelligence Bridge
- Connects to [Descartes Produit](https://github.com/catclever/descartes-produit) and [Dalembert](https://github.com/catclever/Dalembert) over WebSocket (`wss://` or `ws://`).
- Exposes cloud intelligence tools directly to the local IDE:
  - `survey_architecture(query, topic)`: Discovers authoritative open-source repositories and extracts architectural blueprints.
  - `search_code_examples(query, lang)`: Retrieves real-world production code snippets from GitHub.
  - `fetch_doc(package_or_url)`: Pulls sanitized Markdown documentation.
  - `inspect_spec(repo, symbol)`: Retrieves test specs as executable living documentation.

### Module D: Anti-Leak Security Jail
- **Workspace Scoping**: Restricts all file I/O to `$PWD`. Absolute path escapes or `../../` directory traversals are strictly rejected.
- **Sensitive File Blocklist**: Automatically blocks reads to `.env*`, `id_rsa*`, `*.pem`, `*.key`, `.git/credentials`, `~/.aws`, `~/.ssh`.
- **Token Redaction**: Regular expression interceptor masks API keys (e.g. `ghp_*`, `sk-*`, `AKIA*`) with `[REDACTED_SECRET]` before transmission.

---

## 4. Technical Stack & Dependencies

| Component | Choice | Justification |
| :--- | :--- | :--- |
| **Language** | Rust (Edition 2021+) | Zero runtime, maximum execution speed, strong type safety. |
| **Async Runtime** | `tokio` (features = ["full"]) | Industry standard asynchronous runtime for high-performance I/O. |
| **Serialization** | `serde`, `serde_json` | Fast JSON serialization/deserialization for MCP & WebSocket. |
| **CLI Parser** | `clap` (derive) | Declarative, user-friendly CLI argument parsing. |
| **AST Parsing** | `tree-sitter` | Native C/Rust grammar bindings with sub-millisecond query performance. |
| **Grammar Crates** | `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-typescript`, `tree-sitter-python`, `tree-sitter-ruby` | Language-specific AST definitions. |
| **WebSocket** | `tokio-tungstenite` | Robust async WebSocket client with TLS support. |
| **Logging** | `tracing`, `tracing-subscriber` | Structured logging (directed to `stderr` to preserve clean `stdout` for MCP JSON-RPC). |

---

## 5. Development Milestones & Roadmap

- [ ] **Milestone 1: Scaffold & Stdio MCP Server**
  - Setup Cargo workspace with clean error handling (`thiserror`, `anyhow`).
  - Implement JSON-RPC 2.0 loop reading from `stdin` and writing to `stdout`.
  - Expose basic ping/echo and file status tools to verify integration with Cursor.
- [ ] **Milestone 2: Tree-sitter AST Symbol Engine**
  - Integrate Tree-sitter core and grammar crates.
  - Implement `get_outline`, `read_symbol`, and `replace_symbol`.
  - Add comprehensive unit tests verifying that line modifications do not cause drift.
- [ ] **Milestone 3: Cloud Intelligence WebSocket Client**
  - Implement async WebSocket client connecting to `descartes-produit` / `Dalembert`.
  - Implement bidirectional tool proxying and cloud research query delegation.
- [ ] **Milestone 4: Security Jail & Packaging**
  - Implement path normalization and secret redactor.
  - Configure GitHub Actions for multi-platform binary compilation (macOS Apple Silicon/Intel, Linux x86_64/aarch64).
