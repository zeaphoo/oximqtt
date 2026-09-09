[**English**](testing-report.md) | [简体中文](../zh_CN/testing-report.md)

# OXIMQTT Test Report

This document provides the detailed test results for the OXIMQTT MQTT broker, including interoperability testing against the paho.mqtt.testing suite, and performance benchmark data.

---

## Interoperability Testing

OXIMQTT is tested against the official [paho.mqtt.testing](https://github.com/eclipse/paho.mqtt.testing) interoperability test suite.

### Setup

```bash
git clone https://github.com/eclipse/paho.mqtt.testing.git
cd paho.mqtt.testing/interoperability

# Start OXIMQTT broker (separate terminal)
./target/release/oximqttd
```

### MQTT V3.1.1 — 11/11 Passing

| Test | Result | Notes |
|------|--------|-------|
| `test_retained_messages` | ✅ OK | — |
| `test_zero_length_clientid` | ✅ OK | — |
| `will_message_test` | ✅ OK | — |
| `test_offline_message_queueing` | ✅ OK | — |
| `test_overlapping_subscriptions` | ✅ OK | — |
| `test_keepalive` | ✅ OK | — |
| `test_redelivery_on_reconnect` | ✅ OK | — |
| `test_dollar_topics` | ✅ OK | — |
| `test_unsubscribe` | ✅ OK | — |
| `test_subscribe_failure` | ✅ OK | Requires ACL config: add `["deny", "all", "subscribe", ["test/nosubscribe"]]` at first line of ACL rules in `oximqtt.toml` |
| `test_zero_length_clientid` | ✅ OK | — |

### MQTT V5.0 — 24/24 Passing

| Test | Result | Notes |
|------|--------|-------|
| `test_retained_message` | ✅ OK | — |
| `test_will_message` | ✅ OK | — |
| `test_offline_message_queueing` | ✅ OK | — |
| `test_dollar_topics` | ✅ OK | — |
| `test_unsubscribe` | ✅ OK | — |
| `test_session_expiry` | ✅ OK | — |
| `test_basic` | ✅ OK | — |
| `test_overlapping_subscriptions` | ✅ OK | — |
| `test_redelivery_on_reconnect` | ✅ OK | — |
| `test_payload_format` | ✅ OK | — |
| `test_publication_expiry` | ✅ OK | — |
| `test_subscribe_options` | ✅ OK | — |
| `test_assigned_clientid` | ✅ OK | — |
| `test_subscribe_identifiers` | ✅ OK | — |
| `test_request_response` | ✅ OK | — |
| `test_server_topic_alias` | ✅ OK | — |
| `test_client_topic_alias` | ✅ OK | — |
| `test_maximum_packet_size` | ✅ OK | — |
| `test_keepalive` | ✅ OK | — |
| `test_zero_length_clientid` | ✅ OK | — |
| `test_user_properties` | ✅ OK | — |
| `test_flow_control2` | ✅ OK | — |
| `test_flow_control1` | ✅ OK | — |
| `test_will_delay` | ✅ OK | — |
| `test_server_keep_alive` | ✅ OK | Requires config: set `max_keepalive` to 60 in `oximqtt.toml` |
| `test_subscribe_failure` | ✅ OK | Requires ACL config: same as v3.1.1 |

---

## Integration Test Harness

The `oximqtt-test` crate provides a custom test harness with additional test suites beyond paho:

| Suite | Cases | Description |
|-------|-------|-------------|
| `functional_v3` | 2 | MQTT 3.1 basic operations (connect/disconnect, QoS 0 pub/sub) |
| `functional_v311` | 10 | MQTT 3.1.1 protocol compliance |
| `functional_v5` | 5 | MQTT 5.0 protocol compliance |
| `stress` | 3 | Connection load (100 clients), publish QPS (1000 msgs), fan-out (1→N) |
| `chaos` | 6 | Broker restart, connection storms, reconnect, QoS 1 reliability, slow consumer |

```bash
# Run all test suites
cargo build --release
cargo build -p oximqtt-test --release
./target/release/mqtt_harness --workspace .
```

---

## Benchmark

### Environment

| Item | Content |
|------|---------|
| System | x86_64 GNU/Linux, Rocky Linux 9.2 (Blue Onyx) |
| CPU | Intel(R) Xeon(R) CPU E5-2696 v3 @ 2.30GHz, 72 threads (18 cores × 2 threads × 2 sockets) |
| Memory | DDR3/2333, 128 GB |
| Disk | 2 TB |
| Container | Podman v4.4.1 |
| MQTT Bench | `rmqtt/rmqtt-bench:latest` (v0.1.3) |
| MQTT Broker | `zeaphoo/oximqtt:latest` (v0.22.0) |

*Note: MQTT Bench and MQTT Broker run on the same host.*

### Connection Concurrency

| Metric | Value |
|--------|-------|
| Total Concurrent Clients | 1,000,000 |
| Connection Handshake Rate | 5,500-7,000/s |

### Message Throughput

| Metric | Value |
|--------|-------|
| Subscription Clients | 1,000,000 |
| Publishing Clients | 40 |
| Message Throughput Rate | 150,000 msg/s |

### In-repo `mqtt-bench` validation run

`oximqtt-bench` (binary `mqtt-bench`) is a standalone load generator with its
own MQTT 3.1.1 / 5.0 protocol codec — it does not reuse the broker's codec, so
it exercises the broker the way an external client would. A single-host
validation run (client and broker on the same machine, loopback, 256 B
payloads, broker `nodelay = true`) against the current tree — including the
QoS 2 fixes (spec PUBREL reason `0x02` and inbound flow control), all runs
using default `mqtt-bench` flags — produced:

| Scenario | Result |
|----------|--------|
| Connections (v3.1.1, `-c 20000`) | 20,000/20,000 ok |
| Subscriptions (`-c 10000 -S`) | 10,000/10,000 ok |
| QoS 0 ingest, 200 saturating pubs | ~1.45 M msg/s (~3.1 Gbps) |
| QoS 1 ingest, 200 saturating pubs | ~399 K msg/s, all acked |
| QoS 2 ingest, 200 pubs (default flags) | ~218 K msg/s, 0 disconnect |
| QoS 1 ingest, MQTT 5.0 | ~370 K msg/s |
| QoS 2 ingest, MQTT 5.0 (reason 0x02) | ~204 K msg/s, 0 disconnect |
| QoS 1 e2e 1:1, 100 conns, paced | 600,672 sent = received, p50 ≈ 0.9 ms |
| QoS 2 e2e 1:1, 30 conns, v5, paced | 36,163 sent = received, p50 ≈ 0.3 ms |
| Churn `-T` 500 conns, `-D 0.2` | ~91 K msg/s sustained across reconnects |

Interoperability findings surfaced by the independent codec (verified with
byte-level probes) — **both fixed in this tree**:

1. **MQTT 5.0 QoS 2 PUBREL reason code**: the codec only knew `Success = 0`
   and `PacketIdNotFound = 146`, so the spec-mandated `0x02` (Send Onward,
   §3.4.4.1) was treated as a malformed packet and the connection was closed —
   invisible to in-repo clients that share the codec. `SendOnward = 2` is now
   part of `PublishAck2Reason`, and the broker emits `0x02` on its own PUBRELs.
2. **QoS 2 inflight cap**: the per-connection `listener.*.max_inflight`
   (default 16) used to disconnect clients that exceeded it. The inbound QoS 2
   path now applies flow control (deferred PUBREC, drained as PUBRELs arrive)
   instead of dropping the connection. The tool honours the v5 Receive Maximum
   advertisement automatically.

---

## License

Apache-2.0
