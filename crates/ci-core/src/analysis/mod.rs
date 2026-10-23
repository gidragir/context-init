//! Deterministic project analysis pipeline: detect → (optional LLM enrichment in
//! `ci-llm`) → generate/update the L0–L2 knowledge base. Free of any LLM dependency.

pub mod detector;
pub mod types;

pub use types::{DiscoveredModule, McpCheckResult, ProjectAnalysis};
