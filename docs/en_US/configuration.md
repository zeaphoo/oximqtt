# Configuration Reference

OXIMQTT uses a single configuration file (`oximqtt.toml`) with the following sections.

## Configuration Loading Order

Later sources override earlier ones:

1. `/etc/oximqtt/oximqtt.{toml,json,...}` (optional)
2. `/etc/oximqtt.{toml,json,...}` (optional)
3. `./oximqtt.{toml,json,...}` (optional)
4. `-f` / `--config` CLI flag (optional)
5. `OXIMQTT_*` environment variables

## CLI Arguments

| Flag | Type | Description |
|------|------|-------------|
| `-f`, `--config` | `String` | Config file path |
| `-V`, `--version` | flag | Print version info |
| `--id` | `u64` | Override node ID |

---

## `[task]` — Global Task Executor

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `exec_workers` | `usize` | `1000` | Concurrent task count |
| `exec_queue_max` | `usize` | `300000` | Queue capacity |

---

## `[node]` — Node

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `u64` | `0` | Node ID |

### `[node.busy]` — Busy-state Detection

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `check_enable` | `bool` | `true` | Enable busy-status checking |
| `update_interval` | duration | `"2s"` | Re-evaluation interval |
| `loadavg` | `f32` | `80.0` | System load average threshold (0-100) |
| `cpuloadavg` | `f32` | `90.0` | CPU load threshold (0-100) |
| `handshaking` | `isize` | `0` | Concurrent handshake threshold |

---

## `[log]` — Logging

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `to` | string | `"console"` | Destination: `"off"`, `"console"`, `"file"`, `"both"` |
| `level` | string | `"info"` | Level: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"` |
| `dir` | string | `"/var/log/oximqtt"` | Log file directory |
| `file` | string | `"oximqtt.log"` | Log file name |

---

## `[mqtt]` — Protocol Limits

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `delayed_publish_max` | `usize` | `100000` | Max delayed publish messages |
| `delayed_publish_immediate` | `bool` | `true` | Send immediately when limit exceeded (vs discard) |
| `max_sessions` | `isize` | `0` | Max sessions; `0` = unlimited |

---

## `[listener.*]` — Network Listeners

Listeners are organized by protocol. Each protocol section is a map of named listeners:

```toml
## MQTT/TCP - External TCP Listener
listener.tcp.external.addr = "0.0.0.0:1883"
listener.tcp.external.max_connections = 1024000
listener.tcp.external.allow_anonymous = true

## MQTT/TLS - External TLS Listener
listener.tls.external.addr = "0.0.0.0:8883"
listener.tls.external.cert = "./oximqtt-bin/oximqtt.pem"
listener.tls.external.key = "./oximqtt-bin/oximqtt.key"
```

Supported protocol sections: `listener.tcp`, `listener.tls`, `listener.ws`, `listener.wss`, `listener.quic`.

If no TCP/TLS listeners are configured, a default TCP listener on `0.0.0.0:1883` is created.

### Common Listener Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enable` | `bool` | `true` | Whether this listener is active |
| `addr` | string | `"0.0.0.0:1883"` | Bind address and port |
| `max_connections` | `usize` | `1024000` | Max concurrent connections |
| `max_handshaking_limit` | `usize` | `500` | Max concurrent handshakes |
| `max_packet_size` | bytesize | `"1MB"` | Max MQTT packet size; `0` = unlimited |
| `backlog` | `i32` | `1024` | TCP listen backlog |
| `nodelay` | `bool` | `false` | TCP_NODELAY |
| `reuseaddr` | `bool` | `true` | SO_REUSEADDR |
| `reuseport` | `bool` | — | SO_REUSEPORT |
| `allow_anonymous` | `bool` | `false` | Allow unauthenticated connections |
| `min_keepalive` | `u16` | `0` | Min keepalive (seconds) |
| `max_keepalive` | `u16` | `65535` | Max keepalive (seconds) |
| `allow_zero_keepalive` | `bool` | `true` | Allow keepalive = 0 |
| `keepalive_backoff` | `f32` | `0.75` | Keepalive backoff multiplier (> 0.5) |
| `max_inflight` | `u16` | `16` | Max in-flight QoS 1/2 messages |
| `handshake_timeout` | duration | `"15s"` | Handshake timeout |
| `max_mqueue_len` | `usize` | `1000` | Max message queue length per client |
| `mqueue_rate_limit` | string | `"4294967295,1s"` | Queue ejection rate: `"<count>,<duration>"` |
| `max_clientid_len` | `usize` | `65535` | Max client ID length |
| `max_qos_allowed` | `0/1/2` | `2` | Max QoS level |
| `max_topic_levels` | `usize` | `0` | Max topic levels; `0` = unlimited |
| `session_expiry_interval` | duration | `"2h"` | Default session expiry |
| `max_session_expiry_interval` | duration | `"0s"` | Upper limit for client-requested expiry |
| `message_retry_interval` | duration | `"30s"` | QoS 1/2 retry interval; `0` = no resend |
| `message_expiry_interval` | duration | `"5m"` | Message TTL; `0` = no expiration |
| `max_subscriptions` | `usize` | `0` | Max subscriptions per client; `0` = unlimited |
| `max_topic_aliases` | `u16` | `0` | Max topic aliases (MQTT 5.0); `0` = disabled |
| `limit_subscription` | `bool` | `false` | Enable subscription limiting |
| `delayed_publish` | `bool` | `false` | Enable delayed publish |
| `proxy_protocol` | `bool` | `false` | Enable PROXY protocol |
| `proxy_protocol_timeout` | duration | `"5s"` | PROXY protocol header timeout |

### TLS Fields (tls, wss, quic listeners)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cert` | string | — | Server TLS certificate path (PEM) |
| `key` | string | — | Server TLS private key path (PEM) |
| `client_ca_certs` | string | — | Client CA certificates for mutual TLS |
| `cross_certificate` | `bool` | `false` | Enable mutual TLS |
| `cert_cn_as_username` | `bool` | `false` | Extract username from cert CN |
| `cert_subject_dn_as_username` | `bool` | `false` | Extract username from cert Subject DN |
| `collect_cert_info` | `bool` | `false` | Collect cert info into metadata |
| `idle_timeout` | duration | `"90s"` | QUIC max idle timeout |

---

## `[acl]` — Access Control

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disconnect_if_pub_rejected` | `bool` | `true` | Disconnect on publish rejection |
| `rules` | array | `[]` | Ordered ACL rules |

Rule format: `["allow"|"deny", {user="..."} | "all", "pubsub"|"publish"|"subscribe", ["topic/pattern/#"]]`

---

## `[retainer]` — Retained Messages

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_retained_messages` | `usize` | `0` | Max retained messages; `0` = unlimited |
| `max_payload_size` | bytesize | `"1MB"` | Max payload size |
| `retained_message_ttl` | duration | `"0m"` | TTL; `0` = no expiry |

---

## `[auth_jwt]` — JWT Authentication

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disconnect_if_pub_rejected` | `bool` | `true` | Disconnect on publish rejection |
| `disconnect_if_expiry` | `bool` | `false` | Disconnect on JWT expiry |
| `from` | string | `"password"` | Where to extract JWT from |
| `encrypt` | string | `"hmac-based"` | Encryption method |
| `hmac_secret` | string | `"oximqttsecret"` | HMAC secret key |
| `hmac_base64` | `bool` | `false` | Whether secret is base64-encoded |
| `validate_claims.exp` | `bool` | `true` | Validate expiration claim |

---

## `[sys_topic]` — System Topics

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `publish_qos` | `0/1/2` | `1` | QoS for $SYS messages |
| `publish_interval` | duration | `"1m"` | Publish interval |
| `message_expiry_interval` | duration | `"5m"` | Message TTL |

---

## Value Formats

### Duration

String with unit suffix: `"30s"` (seconds), `"5m"` (minutes), `"2h"` (hours). `0` or `"0s"` disables the timeout.

### Bytesize

String with unit: `"1MB"`, `"512KB"`, `"0"` for unlimited.
