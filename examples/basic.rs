use tracing_callgraph::GraphLayer;
use tracing_subscriber::{prelude::*, registry::Registry};

fn setup_global_subscriber() -> impl Drop {
    let (graph_layer, _guard) = GraphLayer::with_file("./output.dot").unwrap();
    let subscriber = Registry::default().with(graph_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Could not set global default");
    _guard
}

#[tracing::instrument]
fn outer_a() {
    inner()
}

#[tracing::instrument]
fn outer_b() {
    inner()
}

#[tracing::instrument]
fn inner() {}

fn main() {
    let _guard = setup_global_subscriber();
    outer_a();
    outer_b();
}
