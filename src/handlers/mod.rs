// handlers/mod.rs
// Re-exports all handlers for backward compatibility

mod datasets;
mod docs;
mod pools;
mod replication;
mod safety;
mod scrub;
mod snapshots;
mod utility;
mod vdev;

// Re-export all handlers - main.rs uses `use handlers::*`
pub use datasets::*;
pub use docs::*;
pub use pools::*;
pub use replication::*;
pub use safety::*;
pub use scrub::*;
pub use snapshots::*;
pub use utility::*;
pub use vdev::*;
