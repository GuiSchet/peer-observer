use shared::lazy_static::lazy_static;
use shared::prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, Opts, register_histogram_vec,
    register_int_counter_vec,
};

const NAMESPACE: &str = "rpcextractor";

pub const LABEL_RPC_METHOD: &str = "rpc_method";

const RPC_DURATION_BUCKETS: [f64; 12] = [
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

lazy_static! {
    /// Time it took to fetch data from the RPC endpoint.
    pub static ref RPC_FETCH_DURATION: HistogramVec = register_histogram_vec!(
        HistogramOpts::new(
            "rpc_fetch_duration_seconds",
            "Time it took to fetch data from the RPC endpoint."
        )
        .namespace(NAMESPACE)
        .buckets(RPC_DURATION_BUCKETS.to_vec()),
        &[LABEL_RPC_METHOD]
    )
    .expect("Could not create rpc_fetch_duration_seconds metric");

    /// Number of errors while fetching data from the RPC endpoint.
    pub static ref RPC_FETCH_ERRORS: IntCounterVec = register_int_counter_vec!(
        Opts::new(
            "rpc_fetch_errors_total",
            "Number of errors while fetching data from the RPC endpoint."
        )
        .namespace(NAMESPACE),
        &[LABEL_RPC_METHOD]
    )
    .expect("Could not create rpc_fetch_errors_total metric");

    /// Number of errors while publishing events to NATS.
    pub static ref NATS_PUBLISH_ERRORS: IntCounterVec = register_int_counter_vec!(
        Opts::new(
            "nats_publish_errors_total",
            "Number of errors while publishing events to NATS."
        )
        .namespace(NAMESPACE),
        &[LABEL_RPC_METHOD]
    )
    .expect("Could not create nats_publish_errors_total metric");
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
        // Accessing the lazy_static metrics should not panic.
        // This verifies that the metrics are registered correctly with the global registry.
        let _ = RPC_FETCH_DURATION.with_label_values(&["test_method"]);
        let _ = RPC_FETCH_ERRORS.with_label_values(&["test_method"]);
        let _ = NATS_PUBLISH_ERRORS.with_label_values(&["test_method"]);
    }

    #[test]
    fn test_valid_label_values() {
        // Verify that all RPC method names are valid labels.
        // Invalid labels would cause a panic when accessing with_label_values().
        for method in RPC_METHODS {
            let _ = RPC_FETCH_DURATION.with_label_values(&[method]);
            let _ = RPC_FETCH_ERRORS.with_label_values(&[method]);
            let _ = NATS_PUBLISH_ERRORS.with_label_values(&[method]);
        }
    }

    #[test]
    fn test_histogram_timer() {
        // Verify that the histogram timer records observations.
        let method = "test_histogram_timer";
        let histogram = RPC_FETCH_DURATION.with_label_values(&[method]);
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
        let method = "test_counter_increment";

        // Test RPC_FETCH_ERRORS counter
        let rpc_error_counter = RPC_FETCH_ERRORS.with_label_values(&[method]);
        let rpc_count_before = rpc_error_counter.get();
        rpc_error_counter.inc();
        assert_eq!(
            rpc_error_counter.get(),
            rpc_count_before + 1,
            "RPC error counter should have incremented by 1"
        );

        // Test NATS_PUBLISH_ERRORS counter
        let nats_error_counter = NATS_PUBLISH_ERRORS.with_label_values(&[method]);
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
}
