# Metrics and Tracing

Moltis includes comprehensive observability support through Prometheus metrics and
tracing integration. This document explains how to enable, configure, and use
these features.

## Overview

The metrics system is built on the [`metrics`](https://docs.rs/metrics) crate
facade, which provides a unified interface similar to the `log` crate. When the
`prometheus` feature is enabled, metrics are exported in Prometheus text format
for scraping by Grafana, Prometheus, or other monitoring tools.

All metrics are **feature-gated** — they add zero overhead when disabled.

## Enabling Metrics

### Compile-Time Feature Flags

Metrics are controlled by the `metrics` feature flag on each crate:

```toml
# Enable metrics for the gateway (includes Prometheus export)
moltis-gateway = { version = "0.1", features = ["metrics"] }

# Enable metrics for specific crates
moltis-agents = { version = "0.1", features = ["metrics"] }
moltis-cron = { version = "0.1", features = ["metrics"] }
```

The gateway's `metrics` feature automatically enables `moltis-metrics/prometheus`
for the Prometheus exporter.

### Default Configuration

By default, the gateway binary includes the `metrics` feature. To build without
metrics:

```bash
cargo build --release --no-default-features
```

## Prometheus Endpoint

When metrics are enabled, the gateway exposes a `/metrics` endpoint:

```
GET http://localhost:8080/metrics
```

This endpoint returns metrics in Prometheus text format:

```
# HELP moltis_http_requests_total Total number of HTTP requests handled
# TYPE moltis_http_requests_total counter
moltis_http_requests_total{method="GET",status="200",endpoint="/api/chat"} 42

# HELP moltis_llm_completion_duration_seconds Duration of LLM completion requests
# TYPE moltis_llm_completion_duration_seconds histogram
moltis_llm_completion_duration_seconds_bucket{provider="anthropic",model="claude-3-opus",le="1.0"} 5
```

### Grafana Integration

To scrape metrics with Prometheus and visualize in Grafana:

1. Add moltis to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'moltis'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

2. Import or create Grafana dashboards using the `moltis_*` metrics.

### JSON API

For the web UI dashboard, authenticated JSON endpoints are available:

```
GET /api/metrics          # Full metrics snapshot
GET /api/metrics/summary  # Lightweight counts for navigation badges
```

## Available Metrics

### HTTP Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_http_requests_total` | Counter | method, status, endpoint | Total HTTP requests |
| `moltis_http_request_duration_seconds` | Histogram | method, status, endpoint | Request latency |
| `moltis_http_requests_in_flight` | Gauge | — | Currently processing requests |

### LLM/Agent Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_llm_completions_total` | Counter | provider, model | Total completions requested |
| `moltis_llm_completion_duration_seconds` | Histogram | provider, model | Completion latency |
| `moltis_llm_input_tokens_total` | Counter | provider, model | Input tokens processed |
| `moltis_llm_output_tokens_total` | Counter | provider, model | Output tokens generated |
| `moltis_llm_completion_errors_total` | Counter | provider, model, error_type | Completion failures |
| `moltis_llm_time_to_first_token_seconds` | Histogram | provider, model | Streaming TTFT |

### MCP (Model Context Protocol) Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_mcp_tool_calls_total` | Counter | server, tool | Tool invocations |
| `moltis_mcp_tool_call_duration_seconds` | Histogram | server, tool | Tool call latency |
| `moltis_mcp_tool_call_errors_total` | Counter | server, tool, error_type | Tool call failures |
| `moltis_mcp_servers_connected` | Gauge | — | Active MCP server connections |

### Tool Execution Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_tool_executions_total` | Counter | tool | Tool executions |
| `moltis_tool_execution_duration_seconds` | Histogram | tool | Execution time |
| `moltis_sandbox_command_executions_total` | Counter | — | Sandbox commands run |

### Session Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_sessions_created_total` | Counter | — | Sessions created |
| `moltis_sessions_active` | Gauge | — | Currently active sessions |
| `moltis_session_messages_total` | Counter | role | Messages by role |

### Cron Job Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_cron_jobs_scheduled` | Gauge | — | Number of scheduled jobs |
| `moltis_cron_executions_total` | Counter | — | Job executions |
| `moltis_cron_execution_duration_seconds` | Histogram | — | Job duration |
| `moltis_cron_errors_total` | Counter | — | Failed jobs |
| `moltis_cron_stuck_jobs_cleared_total` | Counter | — | Jobs exceeding 2h timeout |
| `moltis_cron_input_tokens_total` | Counter | — | Input tokens from cron runs |
| `moltis_cron_output_tokens_total` | Counter | — | Output tokens from cron runs |

### Memory/Search Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_memory_searches_total` | Counter | search_type | Searches performed |
| `moltis_memory_search_duration_seconds` | Histogram | search_type | Search latency |
| `moltis_memory_embeddings_generated_total` | Counter | provider | Embeddings created |

### Channel Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_channels_active` | Gauge | — | Loaded channel plugins |
| `moltis_channel_messages_received_total` | Counter | channel | Inbound messages |
| `moltis_channel_messages_sent_total` | Counter | channel | Outbound messages |

### Telegram-Specific Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_telegram_messages_received_total` | Counter | — | Messages from Telegram |
| `moltis_telegram_access_control_denials_total` | Counter | — | Access denied events |
| `moltis_telegram_polling_duration_seconds` | Histogram | — | Message handling time |

### OAuth Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_oauth_flow_starts_total` | Counter | — | OAuth flows initiated |
| `moltis_oauth_flow_completions_total` | Counter | — | Successful completions |
| `moltis_oauth_token_refresh_total` | Counter | — | Token refreshes |
| `moltis_oauth_token_refresh_failures_total` | Counter | — | Refresh failures |

### Skills Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `moltis_skills_installation_attempts_total` | Counter | — | Installation attempts |
| `moltis_skills_installation_duration_seconds` | Histogram | — | Installation time |
| `moltis_skills_git_clone_total` | Counter | — | Successful git clones |
| `moltis_skills_git_clone_fallback_total` | Counter | — | Fallbacks to HTTP tarball |

## Tracing Integration

The `moltis-metrics` crate includes optional tracing integration via the
`tracing` feature. This allows span context to propagate to metric labels.

### Enabling Tracing

```toml
moltis-metrics = { version = "0.1", features = ["prometheus", "tracing"] }
```

### Initialization

```rust
use moltis_metrics::tracing_integration::init_tracing;

fn main() {
    // Initialize tracing with metrics context propagation
    init_tracing();

    // Now spans will add labels to metrics
}
```

### How It Works

When tracing is enabled, span fields are automatically added as metric labels:

```rust
use tracing::instrument;

#[instrument(fields(operation = "fetch_user", component = "api"))]
async fn fetch_user(id: u64) -> User {
    // Metrics recorded here will include:
    // - operation="fetch_user"
    // - component="api"
    counter!("api_calls_total").increment(1);
}
```

### Span Labels

The following span fields are propagated to metrics:

| Field | Description |
|-------|-------------|
| `operation` | The operation being performed |
| `component` | The component/module name |
| `span.name` | The span's target/name |

## Adding Custom Metrics

### In Your Code

Use the `metrics` macros re-exported from `moltis-metrics`:

```rust
use moltis_metrics::{counter, gauge, histogram, labels};

// Simple counter
counter!("my_custom_requests_total").increment(1);

// Counter with labels
counter!(
    "my_custom_requests_total",
    labels::ENDPOINT => "/api/users",
    labels::METHOD => "GET"
).increment(1);

// Gauge (current value)
gauge!("my_queue_size").set(42.0);

// Histogram (distribution)
histogram!("my_operation_duration_seconds").record(0.123);
```

### Feature-Gating

Always gate metrics code to avoid overhead when disabled:

```rust
#[cfg(feature = "metrics")]
use moltis_metrics::{counter, histogram};

pub async fn my_function() {
    #[cfg(feature = "metrics")]
    let start = std::time::Instant::now();

    // ... do work ...

    #[cfg(feature = "metrics")]
    {
        counter!("my_operations_total").increment(1);
        histogram!("my_operation_duration_seconds")
            .record(start.elapsed().as_secs_f64());
    }
}
```

### Adding New Metric Definitions

For consistency, add metric name constants to `crates/metrics/src/definitions.rs`:

```rust
/// My feature metrics
pub mod my_feature {
    /// Total operations performed
    pub const OPERATIONS_TOTAL: &str = "moltis_my_feature_operations_total";
    /// Operation duration in seconds
    pub const OPERATION_DURATION_SECONDS: &str = "moltis_my_feature_operation_duration_seconds";
}
```

Then use them:

```rust
use moltis_metrics::{counter, my_feature};

counter!(my_feature::OPERATIONS_TOTAL).increment(1);
```

## Web UI Dashboard

The gateway includes a built-in metrics dashboard at `/metrics` in the web UI.
This page displays:

- System metrics (uptime, connected clients)
- LLM usage (completions, tokens, latency)
- Tool execution statistics
- MCP server status
- Provider breakdown table

The dashboard fetches data from `/api/metrics` and updates periodically.

## Best Practices

1. **Use consistent naming**: Follow the pattern `moltis_<subsystem>_<metric>_<unit>`
2. **Add units to names**: `_total` for counters, `_seconds` for durations, `_bytes` for sizes
3. **Keep cardinality low**: Avoid high-cardinality labels (like user IDs or request IDs)
4. **Feature-gate everything**: Use `#[cfg(feature = "metrics")]` to ensure zero overhead when disabled
5. **Use predefined buckets**: The `buckets` module has standard histogram buckets for common metric types

## Configuration

Metrics configuration in `moltis.toml`:

```toml
[metrics]
enabled = true  # Enable metrics collection (default: true when feature enabled)
```

Environment variables:

- `RUST_LOG=moltis_metrics=debug` — Enable debug logging for metrics initialization

## Troubleshooting

### Metrics not appearing

1. Verify the `metrics` feature is enabled at compile time
2. Check that the metrics recorder is initialized (happens automatically in gateway)
3. Ensure you're hitting the correct `/metrics` endpoint

### High memory usage

- Check for high-cardinality labels (many unique label combinations)
- Consider reducing histogram bucket counts

### Missing labels

- Ensure labels are passed consistently across all metric recordings
- Check that tracing spans include the expected fields
