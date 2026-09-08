# Changelog

All notable changes to OXIMQTT are documented in this file.

## Unreleased

### Fixed
- Startup no longer panics when the optional `[auth_jwt]` section is absent or
  partially specified (`missing field hmac_base64`); the module is disabled when
  the section is missing, and all fields fall back to documented defaults.
- Router matched every subscription twice, delivering each message to subscribers
  in duplicate; shared-subscription members were also bypassing group selection.
- Quick reconnect with the same client-id could be refused with
  `ServiceUnavailable` while the previous session was still being torn down;
  the handshake now retries the session lock briefly.

### Changed
- Default `retainer.max_retained_messages` is now `10000` (was `0`/unlimited) so
  the RAM-backed retained store is bounded without operator configuration; set `0`
  explicitly to opt out. When the cap is reached, refreshing already-retained
  topics still succeeds and only new topics are dropped.
- `DefaultSharedSubscription` now performs round-robin member selection
  (preferring online members); previously shared subscriptions were accepted
  but never delivered in the default build.
- Broker binary reports configuration errors as clean process exits instead of
  `expect()`-panics; `Settings::instance()` lazily falls back to defaults
  (library mode no longer panics when `Settings::init` was not called).

### Tests
- New `functional_config` e2e suite: minimal-config startup, `[auth_jwt]`
  absent/empty/partial semantics, clean-exit on fatal misconfiguration,
  unknown sections ignored, bounded retained store behavior.
- New `compat` multi-client suite: v3.1/v3.1.1/v5.0 concurrent connections,
  cross-version pub/sub, retained and wildcard delivery, cross-version session
  takeover, mixed crowd fan-out.

