use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Тип проекта, определяемый по ключевым файлам-маркерам
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    TypeScript,
    Python,
    Go,
    DevOps,
    Generic,
}

impl ProjectType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ProjectType::Rust => "Rust (Cargo)",
            ProjectType::TypeScript => "TypeScript / JavaScript (Node/Bun/Deno)",
            ProjectType::Python => "Python (uv/pip/poetry)",
            ProjectType::Go => "Go (Go Modules)",
            ProjectType::DevOps => "Infrastructure / DevOps (Docker/K8s/Ansible)",
            ProjectType::Generic => "Generic / Multi-language",
        }
    }
}

/// Информация о подсистеме для L0 и L1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub l1_path: String,
    pub tags: Vec<String>,
}

/// Полная структура L0 индекса (сохраняется в L0_index.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L0Index {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub project: String,
    pub description: String,
    pub root_path: String,
    pub updated_at: String,
    pub project_type: String,
    pub subsystems: Vec<SubsystemInfo>,
    pub core_rules: Vec<String>,
}

/// Результат предварительного сканирования проекта
#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    pub name: String,
    pub root_dir: PathBuf,
    pub project_type: ProjectType,
    pub description: String,
    pub detected_modules: Vec<String>,
    pub rules: Vec<String>,
}
