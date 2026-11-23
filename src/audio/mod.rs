//! Pure-Rust source probing with lofty and cover-art discovery.

pub mod artwork;
pub mod probe;

pub use artwork::{find_sidecar_cover, load_cover_file};
pub use probe::{probe_source, ProbedSource};
