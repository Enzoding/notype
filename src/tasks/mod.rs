//! Durable local dictation jobs, independent of GPUI and HTTP.
mod journal;
mod model;
pub mod pipeline;
mod repository;
pub use journal::{AudioWriter, CaptureResult};
use model::CHUNK_SECS;
pub use model::{Summary, Task, TaskConfig, MAX_RECORDING_SECS};
#[cfg(test)]
pub(crate) use repository::tests::fixture;
pub use repository::{atomic_write, now, recover, remove_task};
