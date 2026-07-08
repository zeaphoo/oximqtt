//! MQTT Broker Performance Monitoring System
//!
//! Provides comprehensive metrics collection with:
//! - 50+ operational metrics
//! - Thread-safe atomic counters
//! - Categorized event tracking
//! - Serialization support
//!
//! ## Metric Categories
//! 1. **Client Lifecycle**:
//!    - Authentication attempts/successes
//!    - Connection establishment
//!    - Subscription management
//!    - ACL verification
//!
//! 2. **Session Tracking**:
//!    - Creation/resumption
//!    - Subscription changes
//!    - Termination events
//!
//! 3. **Message Processing**:
//!    - Publish/delivery/ack flows
//!    - Message drops
//!    - Non-subscribed messages
//!    - QoS-specific tracking
//!
//! 4. **Message Types**:
//!    - Custom messages
//!    - Admin messages  
//!    - Last Will messages
//!    - System messages
//!    - Bridge messages
//!    - Retained messages
//!
//! ## Key Features
//! - Atomic counters for thread safety
//! - Macro-generated metric operations
//! - Serde serialization support
//! - Categorized counters for detailed analysis
//!
//! ## Implementation Details
//! - Uses AtomicUsize for lock-free counting
//! - Organized by logical categories

use std::sync::atomic::{AtomicUsize, Ordering};

use paste::paste;
use serde::{Deserialize, Serialize};

macro_rules! impl_metrics {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl Clone for $name {
            fn clone(&self) -> Self {
                Self {
                    $($field: AtomicUsize::new(self.$field.load(Ordering::SeqCst)),)*
                }
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    $($field: AtomicUsize::new(0),)*
                }
            }

            $(
                paste! {
                    #[inline]
                    pub fn [<$field _inc>](&self) {
                        self.$field.fetch_add(1, Ordering::SeqCst);
                    }

                    #[inline]
                    pub fn $field(&self) -> usize {
                        self.$field.load(Ordering::SeqCst)
                    }
                }
            )*

            #[inline]
            pub fn to_json(&self) -> serde_json::Value {
                let mut map = serde_json::Map::new();
                $(map.insert(
                    stringify!($field).into(),
                    serde_json::Value::Number(serde_json::Number::from(self.$field.load(Ordering::SeqCst))),
                );)*
                serde_json::Value::Object(map)
            }

            #[inline]
            pub fn add(&mut self, other: &Self) {
                $(self.$field.fetch_add(other.$field.load(Ordering::SeqCst), Ordering::SeqCst);)*
            }
        }
    };
}

/// Central metrics collector for the MQTT broker.
///
/// Tracks 50+ operational counters across client lifecycle, session management,
/// and message processing (publish/delivery/ack flows). All counters use atomic
/// operations for lock-free thread safety. Metrics are organized into categories
/// matching the main broker data flows.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Metrics {
    client_authenticate: AtomicUsize,
    client_auth_anonymous: AtomicUsize,
    client_auth_anonymous_error: AtomicUsize,
    client_handshaking_timeout: AtomicUsize,
    client_connect: AtomicUsize,
    client_connack: AtomicUsize,
    client_connack_auth_error: AtomicUsize,
    client_connack_unavailable_error: AtomicUsize,
    client_connack_error: AtomicUsize,
    client_connected: AtomicUsize,
    client_disconnected: AtomicUsize,
    client_subscribe_check_acl: AtomicUsize,
    client_publish_check_acl: AtomicUsize,
    client_subscribe: AtomicUsize,
    client_unsubscribe: AtomicUsize,
    client_subscribe_error: AtomicUsize,
    client_subscribe_auth_error: AtomicUsize,
    client_publish_auth_error: AtomicUsize,
    client_publish_error: AtomicUsize,

    session_subscribed: AtomicUsize,
    session_unsubscribed: AtomicUsize,
    session_created: AtomicUsize,
    session_resumed: AtomicUsize,
    session_terminated: AtomicUsize,

    messages_publish: AtomicUsize,
    messages_delivered: AtomicUsize,
    messages_acked: AtomicUsize,
    messages_dropped: AtomicUsize,

    messages_publish_custom: AtomicUsize,
    messages_delivered_custom: AtomicUsize,
    messages_acked_custom: AtomicUsize,

    messages_publish_admin: AtomicUsize,
    messages_delivered_admin: AtomicUsize,
    messages_acked_admin: AtomicUsize,

    messages_publish_lastwill: AtomicUsize,
    messages_delivered_lastwill: AtomicUsize,
    messages_acked_lastwill: AtomicUsize,

    messages_publish_system: AtomicUsize,
    messages_delivered_system: AtomicUsize,
    messages_acked_system: AtomicUsize,

    messages_publish_bridge: AtomicUsize,
    messages_delivered_bridge: AtomicUsize,
    messages_acked_bridge: AtomicUsize,

    messages_delivered_retain: AtomicUsize,
    messages_acked_retain: AtomicUsize,

    messages_nonsubscribed: AtomicUsize,
    messages_nonsubscribed_custom: AtomicUsize,
    messages_nonsubscribed_admin: AtomicUsize,
    messages_nonsubscribed_lastwill: AtomicUsize,
    messages_nonsubscribed_system: AtomicUsize,
    messages_nonsubscribed_bridge: AtomicUsize,
}

impl_metrics!(Metrics {
    client_authenticate,
    client_auth_anonymous,
    client_auth_anonymous_error,
    client_handshaking_timeout,
    client_connect,
    client_connack,
    client_connack_auth_error,
    client_connack_unavailable_error,
    client_connack_error,
    client_connected,
    client_disconnected,
    client_subscribe_check_acl,
    client_publish_check_acl,
    client_subscribe,
    client_unsubscribe,
    client_subscribe_error,
    client_subscribe_auth_error,
    client_publish_auth_error,
    client_publish_error,
    session_subscribed,
    session_unsubscribed,
    session_created,
    session_resumed,
    session_terminated,
    messages_publish,
    messages_delivered,
    messages_acked,
    messages_dropped,
    messages_publish_custom,
    messages_delivered_custom,
    messages_acked_custom,
    messages_publish_admin,
    messages_delivered_admin,
    messages_acked_admin,
    messages_publish_lastwill,
    messages_delivered_lastwill,
    messages_acked_lastwill,
    messages_publish_system,
    messages_delivered_system,
    messages_acked_system,
    messages_publish_bridge,
    messages_delivered_bridge,
    messages_acked_bridge,
    messages_delivered_retain,
    messages_acked_retain,
    messages_nonsubscribed,
    messages_nonsubscribed_custom,
    messages_nonsubscribed_admin,
    messages_nonsubscribed_lastwill,
    messages_nonsubscribed_system,
    messages_nonsubscribed_bridge,
});
