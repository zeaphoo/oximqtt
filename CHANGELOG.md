# Changelog

All notable changes to OXIMQTT are documented in this file.

## Unreleased

### Fixed
- MQTT 5.0 QoS 2: spec-mandated `PUBREL` reason code `0x02` (Send Onward) is
  now accepted and emitted; conformant clients were previously disconnected.
- Inbound QoS 2: exceeding the per-connection `max_inflight` window now
  flow-controls the client instead of disconnecting it.
- Missing/partial `[auth_jwt]` config no longer panics at startup.
- Router no longer delivers messages twice via a matched subscription.
- Reconnect with the same client-id is no longer refused while the previous
  session is being torn down.

### Changed
- Default `retainer.max_retained_messages` is now `10000` (was unlimited).
- Default shared subscriptions now round-robin across members.
- Configuration errors exit cleanly instead of panicking.

### Added
- New workspace member `oximqtt-bench`: standalone `mqtt-bench` load generator
  with its own MQTT 3.1.1 / 5.0 codec (QoS 0/1/2, latency percentiles, churn,
  JSON report).

### Tests
- New `functional_config` and `compat` e2e suites.
- `oximqtt-bench`: 80 unit + 21 e2e tests against a live broker.
