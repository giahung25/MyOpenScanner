pub mod models;
pub mod modules;
pub mod pipeline;
pub mod utils;

pub use models::{DocumentCorners, OutputFormat, PaperSize, PipelineConfig, ScanResult, ScanStatus};
pub use pipeline::ScannerPipeline;
