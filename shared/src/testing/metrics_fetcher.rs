//! Utilities for fetching and parsing Prometheus metrics in integration tests.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Fetches metrics from a Prometheus HTTP endpoint using raw TCP.
///
/// # Arguments
/// * `port` - The port to connect to on localhost
/// * `path` - The HTTP path (e.g., "/" or "/metrics")
///
/// # Returns
/// The raw HTTP response as a string, or an error if the connection fails.
pub fn fetch_metrics(port: u16, path: &str) -> Result<String, std::io::Error> {
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, addr
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(String::from_utf8_lossy(&response).to_string())
}

/// Convenience wrapper that fetches from the root path "/".
pub fn fetch_metrics_root(port: u16) -> Result<String, std::io::Error> {
    fetch_metrics(port, "/")
}

/// Extracts the count value from a histogram metric.
///
/// Searches for a line containing `{metric_name}{rpc_method="{label_value}"}` and
/// returns the numeric value at the end of that line.
///
/// # Panics
/// Panics if the metric is not found or the value cannot be parsed.
pub fn get_histogram_count(metrics_raw: &str, metric_name: &str, label_value: &str) -> u64 {
    let search_pattern = format!("{}{{rpc_method=\"{}\"}}", metric_name, label_value);
    metrics_raw
        .lines()
        .find(|line| line.contains(&search_pattern))
        .and_then(|line| {
            line.split_whitespace()
                .last()
                .map(|v| v.parse::<u64>().expect("failed to parse metric value"))
        })
        .expect("metric not found")
}

/// Extracts the value of a counter metric with a specific label.
///
/// Searches for a line starting with `{metric_name}{rpc_method="{label_value}"}` and
/// returns the numeric value at the end of that line.
///
/// # Panics
/// Panics if the metric is not found or the value cannot be parsed.
pub fn get_counter_value(metrics_raw: &str, metric_name: &str, label_value: &str) -> u64 {
    let search_pattern = format!("{}{{rpc_method=\"{}\"}}", metric_name, label_value);
    metrics_raw
        .lines()
        .find(|line| line.starts_with(&search_pattern))
        .and_then(|line| {
            line.split_whitespace()
                .last()
                .map(|v| v.parse::<u64>().expect("failed to parse metric value"))
        })
        .expect("metric not found")
}
