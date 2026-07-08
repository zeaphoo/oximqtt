[**English**](getting-started.md) | [简体中文](../../zh_CN/development/getting-started.md)

# Getting Started with OXIMQTT Development

This guide walks you through setting up a development environment, building OXIMQTT from source, running tests, and understanding the development workflow.

---

## Prerequisites

### Required

- **Rust** 1.89.0+ (install via [rustup](https://rustup.rs/))
- **Git** (for cloning and version management)

No C compiler, cmake, or OpenSSL needed — OXIMQTT uses pure Rust dependencies only.

### Verify Installation

```bash
rustc --version          # should be 1.89.0+
cargo --version          # should be 1.89.0+
```

---

## Clone and Build

### Get the Source

```bash
git clone https://github.com/zeaphoo/oximqtt.git
cd oximqtt
```

### Build OXIMQTT

```bash
# Debug build (fast compilation, for development)
cargo build

# Release build (optimized, for testing and production)
cargo build --release

# Build a specific sub-crate
cargo build -p oximqtt-codec
cargo build -p oximqttd

# Build with all features
cargo build --release --all-features
```

The production binary is at `target/release/oximqttd` (or `oximqttd.exe` on Windows).

### Build Time Optimization

First build compiles all dependencies and can take 10-30 minutes. Subsequent builds are incremental:

```bash
# Use a specific crate for faster iteration during development
cargo build -p oximqtt-codec

# Build only the core library (skip binary)
cargo build -p oximqtt
```

---

## Project Structure

```
oximqtt/
├── Cargo.toml              # Workspace root
├── oximqtt.toml              # Server configuration (for testing)
│
├── oximqtt/                  # Core broker library
│   ├── Cargo.toml          # Features: metrics, stats, grpc, tls, ws, quic...
│   ├── src/                # Source code (25+ modules)
│   └── examples/           # Library mode examples (simple, multi, plugin, tls, ws, quic)
│
├── oximqtt-bin/              # Binary entry point
│   └── src/
│       ├── server.rs       # Main entry: CLI → config → plugins → start
│       ├── logger.rs       # Tracing-based logger setup
│       └── build.rs        # Plugin registration codegen
│
├── oximqtt-codec/            # MQTT protocol codec
│   └── src/
│       ├── v3/             # MQTT v3.1.1
│       ├── v5/             # MQTT v5.0
│       ├── version/        # Version negotiation
│       ├── error.rs        # Error types
│       └── types.rs        # Shared protocol types
│
├── oximqtt-net/              # Network layer
│   └── src/
│       ├── builder.rs      # Builder + Listener + Acceptor
│       ├── stream.rs       # MQTT stream (v3/v5)
│       ├── ws.rs           # WebSocket support
│       ├── quic.rs         # QUIC support
│       └── error.rs        # MqttError
│
├── oximqtt-conf/             # Configuration management
│   └── src/
│       ├── listener.rs     # Listener configuration
│       ├── logging.rs      # Logging configuration
│       └── options.rs      # CLI argument parsing
│
├── oximqtt-utils/            # Shared utilities
│   └── src/
│       ├── counter.rs      # Atomic counter with merge
│       └── lib.rs          # Bytesize, NodeAddr, timers, serde helpers
│
├── oximqtt-macros/           # Procedural macros (Metrics, Plugin)
│   └── src/
│       ├── metrics.rs      # #[derive(Metrics)] — atomic counter generation
│       └── plugin.rs       # #[derive(Plugin)] — PackageInfo trait
│
├── oximqtt-test/             # Test harness
│   └── src/
│       ├── main.rs         # mqtt_harness entry point
│       ├── broker/         # Broker lifecycle management
│       ├── mqtt/           # Custom MQTT clients (v3/v5)
│       ├── framework/      # Test framework (TestCase, scheduler, context)
│       ├── tests/          # Test cases (functional, stress, chaos)
│       └── report/         # Output reports (console, JSON, HTML)
│
├── docs/                   # Documentation
│   ├── en_US/              # English docs (28 files)
│   └── zh_CN/              # Chinese docs (28 files)
│
├── Dockerfile              # Docker build (x86_64)
├── Dockerfile.amd64        # Docker build (AMD64)
├── Dockerfile.aarch64      # Docker build (ARM64)
└── Makefile                # Docker build automation
```

---

## Development Workflow

### 1. Code → Build → Test Loop

```bash
# Edit code, then:
cargo check              # Fast validation (no binary generation)
cargo build -p oximqtt     # Build specific crate
cargo test -p oximqtt-codec # Run unit tests for a crate
```

### 2. Linting

```bash
# Format code
cargo fmt --all

# Lint (zero warnings required)
cargo clippy --all-targets

# If clippy introduces new warnings, fix them before committing
```

### 3. Running the Full Test Suite

```bash
# Build release binary (required by test harness)
cargo build --release

# Run all unit tests
cargo test

# Run integration tests using the test harness
cargo build -p oximqtt-test --release
./target/release/mqtt_harness --workspace .
```

### 4. Manual Testing

```bash
# Start the broker in dev mode
cargo run -p oximqttd

# Or with a specific config
cargo run -p oximqttd -- -f my-config.toml

# Test with mosquitto clients (separate terminal)
mosquitto_sub -h 127.0.0.1 -p 1883 -t "test/#" -v
mosquitto_pub -h 127.0.0.1 -p 1883 -t "test/topic" -m "hello"
```

---

## Understanding Feature Flags

The core library (`oximqtt`) has 15 feature flags. For development, the most commonly used combinations are:

```bash
# Build with specific features
cargo build -p oximqtt --features "metrics,stats"

# Full feature set (what oximqttd uses)
cargo build -p oximqtt --features "full"

# Test a specific feature combination
cargo test -p oximqtt --features "metrics,stats"
```

When adding a new feature:
1. Add it to `oximqtt/Cargo.toml` `[features]` section
2. Gate module imports with `#[cfg(feature = "your-feature")]`
3. Add it to the `full` feature list if it should be included by default in production
4. Update the feature table in documentation

---

## Working with Built-in Modules

The four previously separate plugin crates (oximqtt-acl, oximqtt-auth-jwt, oximqtt-retainer, oximqtt-sys-topic) have been merged into the `oximqtt` core crate as built-in modules. They are configured directly in `oximqtt.toml` under their respective sections:

- `[acl]` — File-based ACL rules
- `[auth_jwt]` — JWT authentication
- `[retainer]` — Retained message storage
- `[sys_topic]` — $SYS system topic publishing

To enable a built-in module, add its configuration section to `oximqtt.toml`. To disable it, comment out or remove the section.

### Extending the Broker

The hook system remains available for extending broker functionality. Built-in modules register their handlers through the same hook system during server initialization. To add custom behavior, implement the `Handler` trait and register it with the hook system.

---

## Debugging Tips

### Enable Debug Logging

```bash
# Console only
RUST_LOG=debug cargo run -p oximqttd

# File output with trace level
RUST_LOG=trace cargo run -p oximqttd -- -f oximqtt.toml
```

### MQTT Packet Tracing

The test harness (`mqtt_harness`) logs MQTT packet hex dumps at debug level:

```bash
RUST_LOG=debug ./target/release/mqtt_harness --workspace .
```

### Common Issues

**`cargo clippy` warnings in new code**:
- Run `cargo clippy --fix` to auto-fix where possible
- Use `#[allow(clippy::xxx)]` only when you have a justifiable reason

---

## Documentation Standards

All new features and changes must include:

1. **Code documentation** — `///` doc comments on all public API items
2. **Module-level docs** — `//!` at the top of each module explaining its purpose
3. **README updates** — Update the corresponding README.md and README-CN.md
4. **Feature docs** — If adding a plugin or significant feature, add a doc in `docs/en_US/` and `docs/zh_CN/`

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for the full contribution guide.
