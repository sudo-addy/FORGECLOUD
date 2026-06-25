use axum::{routing::get, Router};
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

fn main() {
    let conf = Arc::new(GovernorConfigBuilder::default().per_millisecond(200).burst_size(10).finish().unwrap());
    let app = Router::new().route("/", get(|| async { "OK" })).layer(GovernorLayer { config: conf });
}
