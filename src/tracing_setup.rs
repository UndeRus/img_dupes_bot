use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{SdkTracerProvider, Tracer},
    Resource,
};
use tonic::metadata::MetadataMap;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

fn metadata() -> MetadataMap {
    let metadata = MetadataMap::new();
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

    let subscriber = Registry::default()
        .with(filter_otel)
        .with(opentelemetry_layer)
        ;
    tracing::subscriber::set_global_default(subscriber).expect("Setting tracing subscriber failed");
}

pub fn init_tracing() -> impl Fn() -> () {
    // To send traces to jaeger
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317/")
        .with_metadata(metadata())
        .build()
        .expect("Failed to init otlp exporter");

    global::set_text_map_propagator(TraceContextPropagator::new());

    // To connect traces to exporter
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource())
        .build();
    init_tracing_subscriber(provider.tracer("img dupes bot"));

    global::set_tracer_provider(provider.clone());

    return Box::new(move || {
        provider.shutdown().unwrap();
    });
}
