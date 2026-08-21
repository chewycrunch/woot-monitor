use std::env;

use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Timestamp format carried over from the previous hand-rolled logger.
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Default filter used when `RUST_LOG` is unset.
const DEFAULT_FILTER: &str = "woot_monitor=info,offers=info";

/// Installs the global tracing subscriber. Call once, at the top of `main`.
///
/// Filtering is controlled by `RUST_LOG`, e.g.
/// `RUST_LOG=woot_monitor=debug,offers=info,reqwest=warn`. New-offer events are
/// emitted under the `offers` target so they can be filtered independently of
/// the monitor's own chatter.
///
/// Set `LOG_FORMAT=json` to emit one structured JSON object per event instead
/// of the human-readable format.
pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let json = env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json"));

    let registry = tracing_subscriber::registry().with(filter);

    if json {
        registry
            .with(
                fmt::layer()
                    .json()
                    .with_timer(ChronoLocal::new(TIME_FORMAT.to_string())),
            )
            .init();
    } else {
        registry
            .with(fmt::layer().with_timer(ChronoLocal::new(TIME_FORMAT.to_string())))
            .init();
    }
}
