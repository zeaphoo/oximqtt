[English](README.md) | [**简体中文**](README-CN.md)

# oximqttd

[![crates.io page](https://img.shields.io/crates/v/oximqtt.svg)](https://crates.io/crates/oximqtt)
![Rust](https://img.shields.io/badge/rust-1.89%2B-blue)

OXIMQTT MQTT Broker 的官方二进制入口。

## 实际功能

- **启动流程**：解析 CLI 参数 → 初始化 `oximqtt::conf::Settings` 单例 → 安装 rustls 加密后端 → 初始化 tracing 日志 → 创建 `oximqtt::context::ServerContext` → 初始化内置模块（ACL、Retainer、JWT 认证、Sys Topic） → 绑定配置的监听器 → 启动 MQTT 服务
- **监听器类型**：TCP、TLS、WebSocket (WS)、TLS-WebSocket (WSS)、QUIC
- **信号处理**：Windows 上监听 `Ctrl+C`；非 Windows 上监听 `SIGTERM` + `SIGINT`，收到信号后 100ms 延时退出
- **日志**：通过 `oximqtt::conf::logging::Log` 配置，支持 `off/console/file/both` 模式，UTC+8 时间戳，非阻塞文件写入
- **高性能**：使用 Rust 构建，保证内存安全与运行速度

## 构建

```bash
cargo build -p oximqttd --release
# 产物: target/release/oximqttd (或 oximqttd.exe)
```

## 运行

```bash
./target/release/oximqttd
./target/release/oximqttd -f /path/to/oximqtt.toml
./target/release/oximqttd --config /path/to/oximqtt.toml
./target/release/oximqttd --id 1
```

## 命令行参数

通过 `oximqtt::conf::Options` 结构体（`clap::Parser`）定义：

| 参数 | 类型 | 说明 |
|------|------|------|
| `-f`, `--config` | `Option<String>` | 配置文件路径 |
| `-V`, `--version` | `bool` | 打印版本信息 |
| `--id` | `Option<u64>` | 节点 ID |

## 配置

通过 `oximqtt::conf::Settings` 加载配置。默认路径按优先级：

1. `/etc/oximqtt/oximqtt.{toml,json,...}`（可选）
2. `/etc/oximqtt.{toml,json,...}`（可选）
3. `./oximqtt.{toml,json,...}`（可选）
4. `-f` / `--config` 指定的文件（可选）
5. `OXIMQTT_*` 环境变量

## 相关 crate

- [oximqtt] — 核心 MQTT Broker 库

[oximqtt]: https://crates.io/crates/oximqtt

## 许可证

MIT OR Apache-2.0
