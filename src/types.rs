pub use ci_core::analysis::{DiscoveredModule, McpCheckResult, ProjectAnalysis};
pub use ci_core::storage::{ProjectCommands, ProjectType, SubsystemFile, SubsystemInfo};
use serde::{Deserialize, Serialize};

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

/// Параметры MCP-инструмента context_get_subsystem
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpGetSubsystemParams {
    /// Идентификатор подсистемы из L0_index (например: core, detector, generator)
    pub subsystem_id: String,
    /// Путь к проекту (по умолчанию ".")
    #[serde(default)]
    pub path: Option<String>,
}
