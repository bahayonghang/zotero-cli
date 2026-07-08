//! Shared network client defaults.
//!
//! Both HTTP stacks — zot-remote's async `HttpRuntime` and zot-local's
//! one-shot blocking Pdfium bootstrap downloader — must use these constants
//! so timeout and User-Agent behavior cannot drift between them. The stacks
//! themselves stay separate (zot-local must not depend on zot-remote).

use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const USER_AGENT: &str = concat!("zot-cli/", env!("CARGO_PKG_VERSION"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_defaults_are_pinned() {
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(15));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(60));
        assert!(USER_AGENT.starts_with("zot-cli/"));
    }
}
