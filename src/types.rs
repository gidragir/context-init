use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// Информация о файле внутри подсистемы
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubsystemFile {
    pub path: String,
    pub is_entrypoint: bool,
    pub lines: usize,
}

/// Информация о подсистеме для L0 и L1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubsystemInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub l1_path: String,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SubsystemFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_symbols: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_command: Option<String>,
    #[serde(default)]
    pub tokens: usize,
}

/// Обнаруженные команды разработки
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectCommands {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, String>,
}

/// Полная структура L0 индекса (сохраняется в L0_index.json)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct L0Index {
    #[serde(default, rename = "$schema")]
    pub schema: String,
    pub project: String,
    #[serde(default)]
    pub description: String,
    pub root_path: String,
    pub updated_at: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<String>,
    #[serde(default)]
    pub commands: ProjectCommands,
    pub subsystems: Vec<SubsystemInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub core_rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stow_packages: Vec<String>,
}

/// Результат детального сканирования отдельного модуля
#[derive(Debug, Clone)]
pub struct DiscoveredModule {
    pub dir_path: String,
    pub name: String,
    pub files: Vec<SubsystemFile>,
    pub entrypoints: Vec<String>,
    pub symbols: Vec<String>,
    pub test_cmd: Option<String>,
}

/// Результат предварительного сканирования проекта
#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    pub name: String,
    pub root_dir: PathBuf,
    pub project_type: ProjectType,
    pub description: String,
    pub detected_modules: Vec<DiscoveredModule>,
    pub root_entrypoints: Vec<String>,
    pub commands: ProjectCommands,
    pub workspace_members: Vec<String>,
    pub rules: Vec<String>,
}

/// Результат проверки актуальности базы знаний
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DriftReport {
    pub missing_in_fs: Vec<String>,
    pub untracked_in_knowledge: Vec<String>,
    pub up_to_date_count: usize,
}

/// Параметры MCP-инструмента context_check
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpCheckParams {
    /// Путь к анализируемому проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
}

/// Результат MCP-инструмента context_check
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpCheckResult {
    /// Флаг наличия архитектурного дрейфа
    pub has_drift: bool,
    /// Файлы/подсистемы, удаленные на диске, но оставшиеся в базе знаний
    pub missing_in_fs: Vec<String>,
    /// Новые модули/директории на диске, отсутствующие в базе знаний
    pub untracked_in_knowledge: Vec<String>,
    /// Количество актуальных подсистем
    pub up_to_date_count: usize,
    /// Сводка одной строкой
    pub message: String,
}

/// Параметры MCP-инструмента context_pack
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpPackParams {
    /// Идентификатор подсистемы из L0_index (например: core, detector, generator или all)
    pub subsystem_id: String,
    /// Путь к анализируемому проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
    /// Сжимать контекст (удалять тела функций и нормализовать whitespace) [по умолчанию: true]
    #[serde(default)]
    pub compress: Option<bool>,
    /// Формат вывода: xml или markdown (по умолчанию xml)
    #[serde(default)]
    pub style: Option<String>,
}

/// Параметры MCP-инструмента context_update
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpUpdateParams {
    /// Путь к проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
    /// Глубокое сканирование AST
    #[serde(default)]
    pub deep: Option<bool>,
    /// Обогащение описаний через Ollama
    #[serde(default)]
    pub ai: Option<bool>,
    /// Модель Ollama для AI-обогащения
    #[serde(default)]
    pub model: Option<String>,
}

/// Параметры MCP-инструмента context_init
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpInitParams {
    /// Путь к проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
    /// Глубокое сканирование AST
    #[serde(default)]
    pub deep: Option<bool>,
    /// Обогащение описаний через Ollama
    #[serde(default)]
    pub ai: Option<bool>,
    /// Перезаписать существующую базу знаний
    #[serde(default)]
    pub force: Option<bool>,
    /// Сгенерировать корневой AGENTS.md
    #[serde(default)]
    pub with_agents_md: Option<bool>,
}

/// Параметры MCP-инструмента context_get_subsystems
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpGetSubsystemsParams {
    /// Путь к проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
}
