use dotenvy::dotenv;
use img_hashing_bot::tracing_setup::init_tracing;

fn main() {
    dotenv().ok();

    let finisher = init_tracing(
        &dotenvy::var("OTLP_ENDPOINT").unwrap(),
        &dotenvy::var("OTLP_TOKEN").unwrap(),
    );

    finisher();
}
