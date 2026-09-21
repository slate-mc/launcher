use crate::config::ObservabilityConfig;
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{KeyValue, global};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{Protocol, WithExportConfig as _};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

const SERVICE_NAME: &str = "slate-modpack-api";

pub struct Observability {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

struct ApiMetrics {
    requests: Counter<u64>,
    duration: Histogram<f64>,
    provider_requests: Counter<u64>,
    provider_duration: Histogram<f64>,
    provider_errors: Counter<u64>,
    cache_hits: Counter<u64>,
    cache_misses: Counter<u64>,
    install_plans: Counter<u64>,
}

static API_METRICS: OnceLock<ApiMetrics> = OnceLock::new();

pub fn init(config: &ObservabilityConfig) -> anyhow::Result<Observability> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt = tracing_subscriber::fmt::layer().json();

    if !config.enabled {
        tracing_subscriber::registry().with(filter).with(fmt).init();
        return Ok(Observability {
            tracer_provider: None,
            meter_provider: None,
            logger_provider: None,
        });
    }

    let resource = Resource::builder()
        .with_service_name(SERVICE_NAME)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .with_attribute(KeyValue::new(
            "deployment.environment.name",
            config.environment.clone(),
        ))
        .build();
    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            config.sample_ratio,
        ))))
        .with_resource(resource.clone())
        .build();
    let tracer = tracer_provider.tracer(SERVICE_NAME);

    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()?;
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(resource.clone())
        .build();
    global::set_meter_provider(meter_provider.clone());

    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()?;
    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource)
        .build();
    let log_bridge = OpenTelemetryTracingBridge::new(&logger_provider);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(log_bridge)
        .init();

    tracing::info!(
        sample_ratio = config.sample_ratio,
        environment = %config.environment,
        "OpenTelemetry export enabled"
    );
    Ok(Observability {
        tracer_provider: Some(tracer_provider),
        meter_provider: Some(meter_provider),
        logger_provider: Some(logger_provider),
    })
}

pub fn record_http_request(method: &str, route: &str, status: u16, duration_ms: f64) {
    let metrics = metrics();
    let attributes = [
        KeyValue::new("http.request.method", method.to_owned()),
        KeyValue::new("http.route", route.to_owned()),
        KeyValue::new("http.response.status_code", i64::from(status)),
    ];
    metrics.requests.add(1, &attributes);
    metrics.duration.record(duration_ms, &attributes);
    if route.ends_with("/install-plan") && (200..300).contains(&status) {
        metrics.install_plans.add(1, &attributes);
    }
}

pub fn record_provider_request(upstream: &'static str, succeeded: bool, duration_ms: f64) {
    let metrics = metrics();
    let attributes = [
        KeyValue::new("server.address", upstream),
        KeyValue::new("outcome", if succeeded { "success" } else { "error" }),
    ];
    metrics.provider_requests.add(1, &attributes);
    metrics.provider_duration.record(duration_ms, &attributes);
    if !succeeded {
        metrics.provider_errors.add(1, &attributes);
    }
}

pub fn record_cache_lookup(cache: &'static str, hit: bool) {
    let metrics = metrics();
    let attributes = [KeyValue::new("cache.name", cache)];
    if hit {
        metrics.cache_hits.add(1, &attributes);
    } else {
        metrics.cache_misses.add(1, &attributes);
    }
}

fn metrics() -> &'static ApiMetrics {
    API_METRICS.get_or_init(|| {
        let meter = global::meter(SERVICE_NAME);
        ApiMetrics {
            requests: meter.u64_counter("http_requests_total").build(),
            duration: meter
                .f64_histogram("http_request_duration")
                .with_unit("ms")
                .build(),
            provider_requests: meter.u64_counter("provider_requests_total").build(),
            provider_duration: meter
                .f64_histogram("provider_request_duration")
                .with_unit("ms")
                .build(),
            provider_errors: meter.u64_counter("provider_errors_total").build(),
            cache_hits: meter.u64_counter("cache_hits_total").build(),
            cache_misses: meter.u64_counter("cache_misses_total").build(),
            install_plans: meter.u64_counter("install_plans_total").build(),
        }
    })
}

impl Observability {
    pub fn shutdown(self) {
        if let Some(provider) = self.meter_provider
            && let Err(error) = provider.shutdown()
        {
            tracing::warn!(error = %error, "metric exporter did not shut down cleanly");
        }
        if let Some(provider) = self.logger_provider
            && let Err(error) = provider.shutdown()
        {
            tracing::warn!(error = %error, "log exporter did not shut down cleanly");
        }
        if let Some(provider) = self.tracer_provider
            && let Err(error) = provider.shutdown()
        {
            tracing::warn!(error = %error, "trace exporter did not shut down cleanly");
        }
    }
}
