[English](README.md) | [**简体中文**](README-CN.md)

# oximqtt-macros

[![crates.io page](https://img.shields.io/crates/v/oximqtt-macros.svg)](https://crates.io/crates/oximqtt-macros)
[![docs.rs page](https://docs.rs/oximqtt-macros/badge.svg)](https://docs.rs/oximqtt-macros/latest/oximqtt_macros)

OXIMQTT 生态的过程宏。通过 feature 门控。

## 派生宏

### `#[derive(Metrics)]` — feature `metrics`

为每个字段生成基于 `AtomicUsize` 的计数器：

```rust,ignore
impl MyStruct {
    pub fn new() -> Self;                              // 全零初始化
    pub fn {field}_inc(&self);                         // fetch_add(1, SeqCst)
    pub fn {field}(&self) -> usize;                    // load(SeqCst)
    pub fn to_json(&self) -> serde_json::Value;        // {"field.name": value, ...}
    pub fn add(&mut self, other: &Self);               // 原子合并
    pub fn build_prometheus_metrics(&self, label: &str, gauge_vec: &IntGaugeVec);
}
impl Clone for MyStruct { ... }                        // 通过 AtomicUsize::new(load(...)) 克隆
```

## Cargo.toml

```toml
[dependencies]
oximqtt-macros = { version = "0.1", features = ["metrics"] }
```

## 许可证

MIT OR Apache-2.0
