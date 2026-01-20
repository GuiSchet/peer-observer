use shared::prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry,
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
};

const NAMESPACE: &str = "rpcextractor";

pub const LABEL_RPC_METHOD: &str = "rpc_method";

const RPC_DURATION_BUCKETS: [f64; 12] = [
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Metrics for the rpc-extractor.
/// Each instance has its own registry, allowing for parallel testing.
#[derive(Debug, Clone)]
pub struct Metrics {
    pub registry: Registry,
    /// Time it took to fetch data from the RPC endpoint.
    pub rpc_fetch_duration: HistogramVec,
    /// Number of errors while fetching data from the RPC endpoint.
    pub rpc_fetch_errors: IntCounterVec,
    /// Number of errors while publishing events to NATS.
    pub nats_publish_errors: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new_custom(Some(NAMESPACE.to_string()), None)
            .expect("Could not create prometheus registry");

        let rpc_fetch_duration = register_histogram_vec_with_registry!(
            HistogramOpts::new(
                "rpc_fetch_duration_seconds",
                "Time it took to fetch data from the RPC endpoint."
            )
            .buckets(RPC_DURATION_BUCKETS.to_vec()),
            &[LABEL_RPC_METHOD],
            registry
        )
        .expect("Could not create rpc_fetch_duration_seconds metric");

        let rpc_fetch_errors = register_int_counter_vec_with_registry!(
            Opts::new(
                "rpc_fetch_errors_total",
                "Number of errors while fetching data from the RPC endpoint."
            ),
            &[LABEL_RPC_METHOD],
            registry
        )
        .expect("Could not create rpc_fetch_errors_total metric");

        let nats_publish_errors = register_int_counter_vec_with_registry!(
            Opts::new(
                "nats_publish_errors_total",
                "Number of errors while publishing events to NATS."
            ),
            &[LABEL_RPC_METHOD],
            registry
        )
        .expect("Could not create nats_publish_errors_total metric");

        Self {
            registry,
            rpc_fetch_duration,
            rpc_fetch_errors,
            nats_publish_errors,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All RPC method names used as labels in the metrics.
    const RPC_METHODS: &[&str] = &[
        "getpeerinfo",
        "getmempoolinfo",
        "uptime",
        "getnettotals",
        "getmemoryinfo",
        "getaddrmaninfo",
        "getchaintxstats",
        "getnetworkinfo",
        "getblockchaininfo",
    ];

    #[test]
    fn test_metric_registration() {
        // Creating a new Metrics instance should not panic.
        // This verifies that the metrics are registered correctly with the custom registry.
        let metrics = Metrics::new();
        let _ = metrics
            .rpc_fetch_duration
            .with_label_values(&["test_method"]);
        let _ = metrics.rpc_fetch_errors.with_label_values(&["test_method"]);
        let _ = metrics
            .nats_publish_errors
            .with_label_values(&["test_method"]);
    }

    #[test]
    fn test_valid_label_values() {
        // Verify that all RPC method names are valid labels.
        // Invalid labels would cause a panic when accessing with_label_values().
        let metrics = Metrics::new();
        for method in RPC_METHODS {
            let _ = metrics.rpc_fetch_duration.with_label_values(&[method]);
            let _ = metrics.rpc_fetch_errors.with_label_values(&[method]);
            let _ = metrics.nats_publish_errors.with_label_values(&[method]);
        }
    }

    #[test]
    fn test_histogram_timer() {
        // Verify that the histogram timer records observations.
        let metrics = Metrics::new();
        let method = "test_histogram_timer";
        let histogram = metrics.rpc_fetch_duration.with_label_values(&[method]);
        let count_before = histogram.get_sample_count();

        {
            let _timer = histogram.start_timer();
            // Timer is dropped here, recording the observation
        }

        let count_after = histogram.get_sample_count();
        assert_eq!(
            count_after,
            count_before + 1,
            "Histogram should have recorded one observation"
        );
    }

    #[test]
    fn test_counter_increment() {
        // Verify that counters increment correctly.
        let metrics = Metrics::new();
        let method = "test_counter_increment";

        // Test rpc_fetch_errors counter
        let rpc_error_counter = metrics.rpc_fetch_errors.with_label_values(&[method]);
        let rpc_count_before = rpc_error_counter.get();
        rpc_error_counter.inc();
        assert_eq!(
            rpc_error_counter.get(),
            rpc_count_before + 1,
            "RPC error counter should have incremented by 1"
        );

        // Test nats_publish_errors counter
        let nats_error_counter = metrics.nats_publish_errors.with_label_values(&[method]);
        let nats_count_before = nats_error_counter.get();
        nats_error_counter.inc();
        assert_eq!(
            nats_error_counter.get(),
            nats_count_before + 1,
            "NATS error counter should have incremented by 1"
        );
    }

    #[test]
    fn test_label_constant() {
        // Verify that the label constant has the expected value.
        assert_eq!(LABEL_RPC_METHOD, "rpc_method");
    }

    #[test]
    fn test_isolated_registries() {
        // Verify that each Metrics instance has an isolated registry.
        // This is important for parallel test execution.
        let metrics1 = Metrics::new();
        let metrics2 = Metrics::new();

        // Increment a counter in metrics1
        metrics1.rpc_fetch_errors.with_label_values(&["test"]).inc();

        // Verify metrics2 is not affected
        assert_eq!(
            metrics2.rpc_fetch_errors.with_label_values(&["test"]).get(),
            0,
            "Second Metrics instance should have isolated counters"
        );
    }
}
