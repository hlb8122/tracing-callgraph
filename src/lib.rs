//! A [tracing](https://github.com/tokio-rs/tracing/) [Layer][`GraphLayer`] for generating a call graphs.
//!
//! # Overview
//!
//! [`tracing`] is a framework for instrumenting Rust programs to collect
//! scoped, structured, and async-aware diagnostics. `tracing-callgraph` provides helpers
//! for consuming `tracing` instrumentation that can later be visualized as a
//! call graph in [Graphviz](http://www.graphviz.org/) `dot` representation.
//!
//! ## Layer Setup
//!
//! ```rust
//! use tracing_callgraph::GraphLayer;
//! use tracing_subscriber::{registry::Registry, prelude::*};
//!
//! fn setup_global_subscriber() -> impl Drop {
//!     let (graph_layer, _guard) = GraphLayer::with_file("./output.dot").unwrap();
//!     let subscriber = Registry::default().with(flame_layer);
//!
//!     tracing::subscriber::set_global_default(subscriber).expect("Could not set global default");
//!     _guard
//! }
//!
//! #[tracing::instrument]
//! fn outer_a() {
//!     inner()
//! }
//!
//! #[tracing::instrument]
//! fn outer_b() {
//!     inner()
//! }
//!
//! #[tracing::instrument]
//! fn inner() {}
//!
//! fn main() {
//!     let _ = setup_global_subscriber();
//!     outer_a();
//!     outer_b();
//! }
//! ```
//!
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]

pub use error::Error;
pub use petgraph::dot::Config;

use error::Kind;
use petgraph::dot::Dot;
use petgraph::graphmap::GraphMap;
use petgraph::Directed;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::span;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

mod error;

type CallGraph = GraphMap<&'static str, usize, Directed>;

/// A `Layer` that records span open events as directed edges in a call graph.
///
/// # Dropping and Flushing
///
/// To ensure all data is flushed when the program exits, `GraphLayer` exposes
/// the [`flush_on_drop`] function, which returns a [`FlushGuard`]. The [`FlushGuard`]
/// will flush the writer when it is dropped. If necessary, it can also be used to manually
/// flush the writer.
#[derive(Debug)]
pub struct GraphLayer {
    graph: Arc<Mutex<CallGraph>>,
    top_node: Option<&'static str>,
}

impl GraphLayer {
    /// Add a top node to the graph.
    pub fn enable_top_node(mut self, name: &'static str) -> Self {
        self = self.disable_top_node();
        self.top_node = Some(name.clone());
        self.graph.lock().unwrap().add_node(name);
        self
    }

    /// Remove the top node to the graph.
    pub fn disable_top_node(mut self) -> Self {
        if let Some(name) = self.top_node.take() {
            self.graph.lock().unwrap().remove_node(name);
        }
        self
    }
}

/// An RAII guard for flushing a writer.
#[must_use]
#[derive(Debug)]
pub struct FlushGuard<W>
where
    W: Write + 'static,
{
    graph: Arc<Mutex<CallGraph>>,
    writer: W,
}

impl<W> FlushGuard<W>
where
    W: Write + 'static,
{
    /// Flush the internal writer, ensuring that the graph is written.
    pub fn flush(&mut self) -> Result<(), Error> {
        let graph = match self.graph.lock() {
            Ok(graph) => graph,
            Err(e) => {
                if !std::thread::panicking() {
                    panic!("{}", e);
                } else {
                    return Ok(());
                }
            }
        };
        writeln!(self.writer, "{:?}", Dot::new(&*graph))
            .map_err(Kind::FlushFile)
            .map_err(Error)?;

        self.writer.flush().map_err(Kind::FlushFile).map_err(Error)
    }
}

impl<W> Drop for FlushGuard<W>
where
    W: Write + 'static,
{
    fn drop(&mut self) {
        match self.flush() {
            Ok(_) => (),
            Err(e) => e.report(),
        }
    }
}

impl GraphLayer {
    /// Returns a new [`GraphLayer`] which constructs the call graph.
    pub fn new() -> Self {
        let graph = CallGraph::new();
        Self {
            graph: Arc::new(Mutex::new(graph)),
            top_node: None,
        }
    }

    /// Returns a [`FlushGuard`] which will flush the `GraphLayer`'s writer when
    /// it is dropped, or when `flush` is manually invoked on the guard.
    pub fn flush_on_drop<W>(&self, writer: W) -> FlushGuard<W>
    where
        W: Write + 'static,
    {
        FlushGuard {
            graph: self.graph.clone(),
            writer,
        }
    }
}

impl GraphLayer {
    /// Constructs a `GraphLayer` that constructs the call graph, and a
    /// `FlushGuard` which writes the graph to a `dot` file when dropped.
    pub fn with_file(path: impl AsRef<Path>) -> Result<(Self, FlushGuard<BufWriter<File>>), Error> {
        let path = path.as_ref();
        let file = File::create(path)
            .map_err(|source| Kind::CreateFile {
                path: path.into(),
                source,
            })
            .map_err(Error)?;
        let writer = BufWriter::new(file);
        let layer = Self::new();
        let guard = layer.flush_on_drop(writer);
        Ok((layer, guard))
    }
}

impl<S> Layer<S> for GraphLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let mut locked = self.graph.lock().unwrap();

        // Add node
        let first = ctx.span(id).expect("expected: span id exists in registry");
        let node_b = first.name();
        locked.add_node(node_b);

        // Find parent node
        let node_a = if let Some(parent) = first.parent() {
            parent.name()
        } else if let Some(name) = self.top_node {
            name
        } else {
            return;
        };

        if let Some(weight) = locked.edge_weight_mut(node_a, node_b) {
            // Increase edge weight
            *weight += 1;
        } else {
            // Add edge
            locked.add_edge(node_a, node_b, 1);
        }
    }
}
