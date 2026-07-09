[English](../../en_US/development/getting-started.md) | [**简体中文**](getting-started.md)

# OXIMQTT 开发入门

本文档指导你搭建开发环境、编译 OXIMQTT、运行测试并了解开发工作流。

---

## 前置要求

### 必需

- **Rust** 1.89.0+（通过 [rustup](https://rustup.rs/) 安装）
- **Git**（用于克隆和管理版本）

无需 C 编译器、cmake 或 OpenSSL — OXIMQTT 完全使用纯 Rust 依赖。

### 验证安装

```bash
rustc --version          # 应为 1.89.0+
cargo --version
```

---

## 克隆与构建

```bash
git clone https://github.com/zeaphoo/oximqtt.git
cd oximqtt

# Debug 构建（快速编译，用于开发）
cargo build

# Release 构建（优化，用于测试和生产）
cargo build --release

# 构建特定子 crate
cargo build -p oximqttd
```

生产二进制文件位于 `target/release/oximqttd`（Windows 上为 `oximqttd.exe`）。

---

## 开发工作流

### 编码 → 构建 → 测试 循环

```bash
cargo check              # 快速验证（不生成二进制）
cargo build -p oximqtt     # 构建特定 crate
cargo test -p oximqtt      # 运行单元测试
```

### 代码检查

```bash
# 格式化
cargo fmt --all

# Lint（必须零警告）
cargo clippy --all-targets
```

### 运行完整测试套件

```bash
# 构建 release 二进制（测试框架需要）
cargo build --release

# 运行所有单元测试
cargo test

# 运行集成测试
cargo build -p oximqtt-test --release
./target/release/mqtt_harness --workspace .
```

### 手动测试

```bash
# 启动开发模式 Broker
cargo run -p oximqttd

# 或使用自定义配置
cargo run -p oximqttd -- -f my-config.toml

# 使用 mosquitto 客户端测试
mosquitto_sub -h 127.0.0.1 -p 1883 -t "test/#" -v
mosquitto_pub -h 127.0.0.1 -p 1883 -t "test/topic" -m "hello"
```

---

## 使用 Feature 标志

核心库（`oximqtt`）有 4 个传输层 feature 标志。其他所有功能（延迟发布、保留消息、指标统计等）均无条件编译：

```bash
# 默认构建（包含 TLS + WebSocket + QUIC）
cargo build -p oximqtt

# 不含默认功能
cargo build -p oximqtt --no-default-features

# 仅启用 TLS
cargo build -p oximqtt --no-default-features --features "tls"
```

---

## 内置模块开发

部分功能模块（acl、retainer、auth_jwt、sys_topic）已合并到 oximqtt 核心 crate 中作为内置模块。如需扩展功能，可通过钩子系统（Hook）实现。

更详细的贡献指南请参阅 [CONTRIBUTING.md](../../CONTRIBUTING.md)。

## 许可证

Apache-2.0
