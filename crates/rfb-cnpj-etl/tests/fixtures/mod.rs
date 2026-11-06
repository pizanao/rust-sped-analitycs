//! Fixtures module for E2E tests.

pub mod generator;

#[allow(unused_imports)]
pub use generator::{
    create_zip_with_content, generate_synthetic_rfb_dataset, GeneratorOptions,
    SyntheticDatasetSummary, CANONICAL_ZIP_FILES,
};
