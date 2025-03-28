use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::{SdkTracerProvider, Tracer},
    Resource,
};
use tonic::metadata::MetadataMap;
use tracing::Level;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{filter, fmt, layer::SubscriberExt, EnvFilter, Layer, Registry};

fn metadata(auth_token: &str) -> MetadataMap {
    let mut metadata = MetadataMap::with_capacity(3);
    metadata.insert(
        "authorization",
        format!("Basic {}", auth_token).parse().unwrap(),
    );
    metadata.insert("organization", "default".parse().unwrap());
    metadata.insert("stream-name", "default".parse().unwrap());
    metadata
}

fn resource() -> Resource {
    Resource::builder()
        .with_service_name("img dupes tgbot")
        .build()
}

fn init_tracing_subscriber(tracer: Tracer) {
    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("opentelemetry=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());

    let opentelemetry_layer = OpenTelemetryLayer::new(tracer);


    let stdout_layer = fmt::layer().with_level(true).with_filter(filter::LevelFilter::from_level(Level::DEBUG));

    let subscriber = Registry::default()
        .with(filter_otel)
        .with(opentelemetry_layer)
        .with(stdout_layer);
    tracing::subscriber::set_global_default(subscriber).expect("Setting tracing subscriber failed");
}

pub fn init_tracing(otlp_endpoint: &str, token: &str) -> impl Fn() -> () {
    // To send traces to jaeger
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_metadata(metadata(token))
        .build()
        .expect("Failed to init otlp tracers exporter");

    global::set_text_map_propagator(TraceContextPropagator::new());


    // To connect traces to exporter
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(resource())
        .build();
    init_tracing_subscriber(tracer_provider.tracer("img dupes bot"));

    global::set_tracer_provider(tracer_provider.clone());
    
    let metrics_exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_metadata(metadata(token))
        .build()
        .expect("Failed to init otlp metrics exporter");

    let metrics_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metrics_exporter)
        .with_resource(resource())
        .build();

    global::set_meter_provider(metrics_provider.clone());
    

    return Box::new(move || {
        tracer_provider
            .shutdown()
            .expect("Failed to shutdown tracer");
        metrics_provider
            .shutdown()
            .expect("Failed to shutdown metrics");
    });
}
