use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace::{SdkTracerProvider, Tracer}, Resource};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, Registry};


fn init_tracer() -> Tracer {
    let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic() // Use gRPC transport
        .with_endpoint("http://localhost:4317") // Set your Jaeger OTLP endpoint
        .build()
        .expect("Failed to build otlp exporter");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("telegram-img-dupes-bot-service")
                .build(),
        )
        .build();

    let tracer = provider.tracer("telegram-img-dupes-bot");
    global::set_tracer_provider(provider);
    tracer
}

pub fn init_tracing() {
    let tracer = init_tracer();
    let telemetry_layer = OpenTelemetryLayer::new(tracer);
    
    let subscriber = Registry::default().with(telemetry_layer);
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
}