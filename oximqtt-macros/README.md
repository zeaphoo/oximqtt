[**English**](README.md) | [简体中文](README-CN.md)

# oximqtt-macros

[![crates.io page](https://img.shields.io/crates/v/oximqtt-macros.svg)](https://crates.io/crates/oximqtt-macros)
[![docs.rs page](https://docs.rs/oximqtt-macros/badge.svg)](https://docs.rs/oximqtt-macros/latest/oximqtt_macros)

Procedural macros for the OXIMQTT ecosystem. Feature-gated.

## Derive macros

### `#[derive(Metrics)]` — feature `metrics`

Generates `AtomicUsize`-based counters for each field. Generated methods:

```rust,ignore
impl MyStruct {
    pub fn new() -> Self;                              // zero-init all fields
    pub fn {field}_inc(&self);                         // fetch_add(1, SeqCst)
    pub fn {field}(&self) -> usize;                    // load(SeqCst)
    pub fn to_json(&self) -> serde_json::Value;        // {"field.name": value, ...}
    pub fn add(&mut self, other: &Self);               // atomic add merge
    pub fn build_prometheus_metrics(&self, label: &str, gauge_vec: &IntGaugeVec);
}
impl Clone for MyStruct { ... }                        // clone via AtomicUsize::new(load(...))
```

## Cargo.toml

```toml
[dependencies]
oximqtt-macros = { version = "0.1", features = ["metrics"] }
```

## License

MIT OR Apache-2.0
