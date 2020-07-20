# tracing-callgraph

A [tracing](https://github.com/tokio-rs/tracing/) library for generating call graphs in [Graphviz](http://www.graphviz.org/) `dot` representation.

[![CI](https://github.com/hlb8122/tracing-callgraph/workflows/CI/badge.svg)](https://github.com/hlb8122/tracing-callgraph/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Cargo](https://img.shields.io/crates/v/tracing-callgraph.svg)](https://crates.io/crates/tracing-callgraph)
[![Documentation](https://docs.rs/tracing-callgraph/badge.svg)](https://docs.rs/tracing-callgraph)

## Example

```rust
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
```

**Output**

```
digraph {
    0 [ label = "\"outer_a\"" ]
    1 [ label = "\"inner\"" ]
    2 [ label = "\"outer_b\"" ]
    0 -> 1 [ label = "1" ]
    2 -> 1 [ label = "1" ]
}
```

### Special Thanks

Special thanks to the authors of [tracing-flame](https://github.com/tokio-rs/tracing/tree/master/tracing-flame) which this draws on heavily.
