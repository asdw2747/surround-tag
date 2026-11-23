pub mod model;
pub mod xml;

pub use model::{AlbumTags, SimpleTag, TagDocument, TagElement, TagPlan, TagPolicy, TrackTags};
pub use xml::{fresh_document, parse, plan_document, render};
