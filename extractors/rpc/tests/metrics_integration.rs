#![cfg(feature = "nats_integration_tests")]
#![cfg(feature = "node_integration_tests")]

use shared::{
    corepc_node,
    log::{self, info},
    nats_util::NatsArgs,
    simple_logger::SimpleLogger,
    testing::nats_server::NatsServerForTesting,
    tokio::{self, sync::watch},
};

use std::net::TcpListener;
use std::sync::Once;

use rpc_extractor::Args;

/// Get an available port for the metrics server.
fn get_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to port 0")
        .local_addr()
        .expect("Failed to get local addr")
        .port()
}

static INIT: Once = Once::new();

// 1 second query interval for fast tests
const QUERY_INTERVAL_SECONDS: u64 = 1;

fn setup() {
    INIT.call_once(|| {
        SimpleLogger::new()
            .with_level(log::LevelFilter::Trace)
            .init()
            .unwrap();
    });
}

#[allow(clippy::too_many_arguments)]
fn make_test_args(
    nats_port: u16,
    rpc_url: String,
    cookie_file: String,
    prometheus_address: String,
    disable_getpeerinfo: bool,
    disable_getmempoolinfo: bool,
    disable_uptime: bool,
    disable_getnettotals: bool,
    disable_getmemoryinfo: bool,
    disable_getaddrmaninfo: bool,
    disable_getchaintxstats: bool,
    disable_getnetworkinfo: bool,
    disable_getblockchaininfo: bool,
) -> Args {
    Args::new(
        NatsArgs {
            address: format!("127.0.0.1:{}", nats_port),
            username: None,
            password: None,
            password_file: None,
        },
        log::Level::Trace,
        rpc_url,
        cookie_file,
        QUERY_INTERVAL_SECONDS,
        prometheus_address,
        disable_getpeerinfo,
        disable_getmempoolinfo,
        disable_uptime,
        disable_getnettotals,
        disable_getmemoryinfo,
        disable_getaddrmaninfo,
        disable_getchaintxstats,
        disable_getnetworkinfo,
        disable_getblockchaininfo,
    )
}

fn setup_node(conf: corepc_node::Conf) -> corepc_node::Node {
    info!("env BITCOIND_EXE={:?}", std::env::var("BITCOIND_EXE"));
    info!("exe_path={:?}", corepc_node::exe_path());

    if let Ok(exe_path) = corepc_node::exe_path() {
        info!("Using bitcoind at '{}'", exe_path);
        return corepc_node::Node::with_conf(exe_path, &conf).unwrap();
    }

    info!("Trying to download a bitcoind..");
    corepc_node::Node::from_downloaded_with_conf(&conf).unwrap()
}

fn setup_two_connected_nodes() -> (corepc_node::Node, corepc_node::Node) {
    // node1 listens for p2p connections
    let mut node1_conf = corepc_node::Conf::default();
    node1_conf.p2p = corepc_node::P2P::Yes;
    let node1 = setup_node(node1_conf);

    // node2 connects to node1
    let mut node2_conf = corepc_node::Conf::default();
    node2_conf.p2p = node1.p2p_connect(true).unwrap();
    let node2 = setup_node(node2_conf);

    (node1, node2)
}

/// Fetches the metrics from the Prometheus endpoint.
fn fetch_metrics(port: u16) -> Result<String, String> {
    let url = format!("http://127.0.0.1:{}/metrics", port);

    match ureq::get(&url).call() {
        Ok(response) => match response.into_string() {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Failed to read response body: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch metrics: {}", e)),
    }
}

/// Extracts the count value from a histogram metric.
fn get_histogram_count(metrics_raw: &str, metric_name: &str, label_value: &str) -> u64 {
    let search_pattern = format!("{}{{rpc_method=\"{}\"}}", metric_name, label_value);
    metrics_raw
        .lines()
        .find(|line| line.contains(&search_pattern))
        .and_then(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts
                .last()
                .map(|v| v.parse::<u64>().expect("failed to parse metric value"))
        })
        .expect("metric not found")
}

/// Extracts the value of a counter metric with a specific label.
fn get_counter_value(metrics_raw: &str, metric_name: &str, label_value: &str) -> u64 {
    let search_pattern = format!("{}{{rpc_method=\"{}\"}}", metric_name, label_value);
    metrics_raw
        .lines()
        .find(|line| line.starts_with(&search_pattern))
        .and_then(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts
                .last()
                .map(|v| v.parse::<u64>().expect("failed to parse metric value"))
        })
        .expect("metric not found")
}

#[tokio::test]
async fn test_integration_metrics_server_basic() {
    setup();
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_server = NatsServerForTesting::new(&[]).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port = get_available_port();

    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            node1.rpc_url().replace("http://", ""),
            node1.params.cookie_file.display().to_string(),
            format!("127.0.0.1:{}", metrics_port),
            true, // disable all RPCs for this basic test
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
        );
        let _ = rpc_extractor::run(args, shutdown_rx.clone()).await;
    });

    // Wait for the metrics server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Fetch metrics and verify the server responds
    let metrics = fetch_metrics(metrics_port);
    assert!(
        metrics.is_ok(),
        "Metrics server should respond: {:?}",
        metrics
    );

    // With per-instance registries, metrics only appear after being accessed.
    // This basic test verifies the server starts and responds.
    // Actual metric content is verified by other tests that make RPC calls.

    shutdown_tx.send(true).unwrap();
    rpc_extractor_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_rpc_fetch_duration() {
    setup();
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_server = NatsServerForTesting::new(&[]).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port = get_available_port();

    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            node1.rpc_url().replace("http://", ""),
            node1.params.cookie_file.display().to_string(),
            format!("127.0.0.1:{}", metrics_port),
            true,  // disable getpeerinfo
            true,  // disable getmempoolinfo
            false, // enable uptime (lightweight RPC)
            true,
            true,
            true,
            true,
            true,
            true,
        );
        let _ = rpc_extractor::run(args, shutdown_rx.clone()).await;
    });

    // Wait for at least one RPC query cycle
    tokio::time::sleep(tokio::time::Duration::from_secs(QUERY_INTERVAL_SECONDS + 1)).await;

    // Fetch metrics and verify duration was recorded
    let metrics = fetch_metrics(metrics_port).expect("Should fetch metrics");

    // Check that the histogram count for uptime is at least 1
    let uptime_count = get_histogram_count(
        &metrics,
        "rpcextractor_rpc_fetch_duration_seconds_count",
        "uptime",
    );
    assert!(
        uptime_count >= 1,
        "Should have recorded at least one uptime RPC call, got: {}",
        uptime_count
    );

    shutdown_tx.send(true).unwrap();
    rpc_extractor_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_all_rpc_methods_duration() {
    setup();
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_server = NatsServerForTesting::new(&[]).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port = get_available_port();

    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            node1.rpc_url().replace("http://", ""),
            node1.params.cookie_file.display().to_string(),
            format!("127.0.0.1:{}", metrics_port),
            false, // enable getpeerinfo
            false, // enable getmempoolinfo
            false, // enable uptime
            false, // enable getnettotals
            false, // enable getmemoryinfo
            false, // enable getaddrmaninfo
            true,  // disable getchaintxstats (runs less frequently)
            false, // enable getnetworkinfo
            true,  // disable getblockchaininfo (runs less frequently)
        );
        let _ = rpc_extractor::run(args, shutdown_rx.clone()).await;
    });

    // Wait for at least one RPC query cycle
    tokio::time::sleep(tokio::time::Duration::from_secs(QUERY_INTERVAL_SECONDS + 1)).await;

    // Fetch metrics
    let metrics = fetch_metrics(metrics_port).expect("Should fetch metrics");

    // Verify that each enabled RPC method has recorded at least one call
    let methods_to_check = [
        "getpeerinfo",
        "getmempoolinfo",
        "uptime",
        "getnettotals",
        "getmemoryinfo",
        "getaddrmaninfo",
        "getnetworkinfo",
    ];

    for method in methods_to_check {
        let count = get_histogram_count(
            &metrics,
            "rpcextractor_rpc_fetch_duration_seconds_count",
            method,
        );
        assert!(
            count >= 1,
            "Should have recorded at least one {} RPC call, got: {}",
            method,
            count
        );
    }

    shutdown_tx.send(true).unwrap();
    rpc_extractor_handle.await.unwrap();
}

#[tokio::test]
async fn test_integration_metrics_rpc_fetch_errors() {
    setup();
    let nats_server = NatsServerForTesting::new(&[]).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port = get_available_port();

    // Create a temporary cookie file (required by Args validation)
    let temp_dir = std::env::temp_dir();
    let cookie_file = temp_dir.join(format!("test_cookie_{}", metrics_port));
    std::fs::write(&cookie_file, "__cookie__:test").expect("Failed to write cookie file");

    let cookie_file_path = cookie_file.display().to_string();
    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            "127.0.0.1:1".to_string(), // Unreachable RPC host
            cookie_file_path,
            format!("127.0.0.1:{}", metrics_port),
            true,  // disable getpeerinfo
            true,  // disable getmempoolinfo
            false, // enable uptime (will fail)
            true,
            true,
            true,
            true,
            true,
            true,
        );
        let _ = rpc_extractor::run(args, shutdown_rx.clone()).await;
    });

    // Wait for at least one RPC query cycle (which will fail)
    tokio::time::sleep(tokio::time::Duration::from_secs(QUERY_INTERVAL_SECONDS + 1)).await;

    // Fetch metrics and verify error counter was incremented
    let metrics = fetch_metrics(metrics_port).expect("Should fetch metrics");

    // Check that the error counter for uptime is at least 1
    let error_count = get_counter_value(&metrics, "rpcextractor_rpc_fetch_errors_total", "uptime");
    assert!(
        error_count >= 1,
        "Should have recorded at least one uptime RPC error, got: {}. Metrics:\n{}",
        error_count,
        metrics
    );

    shutdown_tx.send(true).unwrap();
    rpc_extractor_handle.await.unwrap();

    // Cleanup
    let _ = std::fs::remove_file(&cookie_file);
}

#[tokio::test]
async fn test_integration_metrics_rpc_fetch_errors_invalid_auth() {
    setup();
    let (node1, _node2) = setup_two_connected_nodes();
    let nats_server = NatsServerForTesting::new(&[]).await;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let metrics_port = get_available_port();

    // Create a cookie file with invalid credentials
    let temp_dir = std::env::temp_dir();
    let invalid_cookie_file = temp_dir.join(format!("invalid_cookie_{}", metrics_port));
    std::fs::write(&invalid_cookie_file, "__cookie__:invalid_password")
        .expect("Failed to write invalid cookie file");

    let invalid_cookie_path = invalid_cookie_file.display().to_string();
    let rpc_url = node1.rpc_url().replace("http://", "");
    let rpc_extractor_handle = tokio::spawn(async move {
        let args = make_test_args(
            nats_server.port,
            rpc_url,
            invalid_cookie_path,
            format!("127.0.0.1:{}", metrics_port),
            true,  // disable getpeerinfo
            true,  // disable getmempoolinfo
            false, // enable uptime (will fail due to auth)
            true,
            true,
            true,
            true,
            true,
            true,
        );
        let _ = rpc_extractor::run(args, shutdown_rx.clone()).await;
    });

    // Wait for at least one RPC query cycle (which will fail due to invalid auth)
    tokio::time::sleep(tokio::time::Duration::from_secs(QUERY_INTERVAL_SECONDS + 1)).await;

    // Fetch metrics and verify error counter was incremented
    let metrics = fetch_metrics(metrics_port).expect("Should fetch metrics");

    // Check that the error counter for uptime is at least 1
    let error_count = get_counter_value(&metrics, "rpcextractor_rpc_fetch_errors_total", "uptime");
    assert!(
        error_count >= 1,
        "Should have recorded at least one uptime RPC error due to invalid auth, got: {}. Metrics:\n{}",
        error_count,
        metrics
    );

    shutdown_tx.send(true).unwrap();
    rpc_extractor_handle.await.unwrap();

    // Cleanup
    let _ = std::fs::remove_file(&invalid_cookie_file);
}
