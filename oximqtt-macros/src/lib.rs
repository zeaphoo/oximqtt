#![deny(unsafe_code)]
//! # Procedural Macro Utilities
//!
//! Provides custom derive macros for metrics collection.
//! Features must be explicitly enabled through crate features.
//!
//! ## Available Features
//! - `metrics`: Enables metrics collection derive macro
//!
//! ## Example Usage
//! ```rust,ignore
//! #[cfg(feature = "metrics")]
//! #[derive(Metrics)]
//! struct NetworkMetrics {
//!     bytes_sent: Counter,
//!     bytes_received: Counter,
//! }
//! ```

extern crate proc_macro;
extern crate proc_macro2;

#[cfg(feature = "metrics")]
mod metrics;

/// Derive macro for implementing metrics collection
///
/// # Examples
/// ```rust,ignore
/// #[cfg(feature = "metrics")]
/// #[derive(Metrics)]
/// struct ServiceMetrics {
///     requests: Counter,
///     latency: Histogram,
///     errors: Gauge,
/// }
/// ```
#[cfg(feature = "metrics")]
#[proc_macro_derive(Metrics)]
pub fn derive_metrics(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    metrics::build(input)
}
