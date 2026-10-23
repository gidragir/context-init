//! Language-parameterized prompt templates (`PromptTemplateRegistry`) with filesystem overrides
//! from `<config_dir>/prompts/<role>.md` (`.txt`) and `<repo>/.context-init/prompts/<role>.md` (D17).

use crate::provider::{LlmRequest, RoleName};
use crate::schema::default_role_json_schema;
use anyhow::{Context, Result};
use ci_core::config::{Config, DEFAULT_LANGUAGE};
use ci_core::storage::normalize_line_endings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Origin of a loaded `PromptTemplate` (D17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source_kind", content = "path", rename_all = "snake_case")]
pub enum PromptTemplateSource {
    /// Default built-in template shipped with `ci-llm`.
    BuiltIn,
    /// Overridden by a file in the global configuration directory (`<config_dir>/prompts/<role>.md` or `.txt`).
    GlobalConfig(PathBuf),
    /// Overridden by a file in the repository (`.context-init/prompts/<role>.md` or `.txt`).
    RepoConfig(PathBuf),
    /// Programmatically registered template.
    Custom,
}

/// Template definition for a single execution role (`init`, `update`, `task`, `search`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PromptTemplate {
    /// Target role.
    pub role: RoleName,
    /// System prompt template (supports `{language}`, `{language_instruction}`, `{role}`).
    pub system_template: String,
    /// User prompt template (supports `{language}`, `{language_instruction}`, `{role}`, `{project}`, `{facts}`, `{query}`, `{json_schema}`).
    pub user_template: String,
    /// Origin of this template.
    pub source: PromptTemplateSource,
}

/// Variables substituted when rendering a `PromptTemplate`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PromptVars {
    /// Optional project name or identifier.
    pub project: String,
    /// Grounded repository facts, module structure, or diff summary.
    pub facts: String,
    /// Optional search query or task description.
    pub query: String,
    /// Optional JSON schema string to embed in the prompt.
    pub json_schema: String,
    /// Optional structured facts value to attach to `LlmRequest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_facts: Option<Value>,
}

impl PromptVars {
    /// Creates `PromptVars` with `facts`.
    pub fn with_facts(facts: impl Into<String>) -> Self {
        Self {
            facts: facts.into(),
            ..Self::default()
        }
    }

    /// Sets the `project` variable.
    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    /// Sets the `query` variable.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Sets the `json_schema` string variable.
    pub fn json_schema(mut self, json_schema: impl Into<String>) -> Self {
        self.json_schema = json_schema.into();
        self
    }

    /// Sets the optional `structured_facts` JSON value.
    pub fn structured_facts(mut self, facts: Value) -> Self {
        self.structured_facts = Some(facts);
        self
    }
}

/// Rendered system and user prompts ready for an `LlmRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RenderedPrompt {
    /// Target role.
    pub role: RoleName,
    /// Resolved target natural language code.
    pub language: String,
    /// Rendered system prompt.
    pub system_prompt: String,
    /// Rendered user prompt.
    pub prompt: String,
    /// Provenance of the template that produced this prompt.
    pub source: PromptTemplateSource,
}

/// Registry of role prompt templates parameterized by `Config.language` with support for
/// global (`<config_dir>/prompts/<role>.md`) and project (`<repo>/.context-init/prompts/<role>.md`) overrides (D17).
#[derive(Debug, Clone)]
pub struct PromptTemplateRegistry {
    language: String,
    templates: BTreeMap<RoleName, PromptTemplate>,
}

impl Default for PromptTemplateRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_LANGUAGE)
    }
}

impl PromptTemplateRegistry {
    /// Creates a `PromptTemplateRegistry` initialized with built-in templates for `language`.
    pub fn new(language: impl Into<String>) -> Self {
        let lang = normalize_language(language.into());
        let mut templates = BTreeMap::new();
        for role in [
            RoleName::Init,
            RoleName::Update,
            RoleName::Task,
            RoleName::Search,
        ] {
            templates.insert(role, builtin_template_for_role(role));
        }
        Self {
            language: lang,
            templates,
        }
    }

    /// Creates a `PromptTemplateRegistry` from `Config` and applies any custom template files found in
    /// `config_dir` (`<config_dir>/prompts/<role>.md` or `.txt`) and `repo_root` (`<repo>/.context-init/prompts/<role>.md` or `.txt`).
    pub fn from_config(
        config: &Config,
        config_dir: Option<&Path>,
        repo_root: Option<&Path>,
    ) -> Result<Self> {
        Self::load(&config.language, config_dir, repo_root)
    }

    /// Loads a `PromptTemplateRegistry` for `language`, applying overrides from `config_dir` and `repo_root`.
    ///
    /// Precedence order:
    /// 1. `<repo_root>/.context-init/prompts/<role>.{md,txt}`
    /// 2. `<config_dir>/prompts/<role>.{md,txt}`
    /// 3. Built-in role template
    pub fn load(
        language: impl Into<String>,
        config_dir: Option<&Path>,
        repo_root: Option<&Path>,
    ) -> Result<Self> {
        let mut registry = Self::new(language);
        if let Some(cfg_dir) = config_dir {
            registry.load_global_overrides(cfg_dir)?;
        }
        if let Some(repo) = repo_root {
            registry.load_repo_overrides(repo)?;
        }
        Ok(registry)
    }

    /// Returns the configured target language code (e.g. `"ru"`, `"en"`).
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Updates the target language code used during template rendering.
    pub fn set_language(&mut self, language: impl Into<String>) {
        self.language = normalize_language(language.into());
    }

    /// Returns the template registered for `role`.
    pub fn get(&self, role: RoleName) -> &PromptTemplate {
        self.templates
            .get(&role)
            .expect("all RoleName variants are populated in PromptTemplateRegistry")
    }

    /// Registers a custom `PromptTemplate` for `role`.
    pub fn set(&mut self, role: RoleName, template: PromptTemplate) {
        self.templates.insert(role, template);
    }

    /// Loads global prompt overrides from `<config_dir>/prompts/<role>.{md,txt}` (or `<config_dir>/<role>.{md,txt}`).
    pub fn load_global_overrides(&mut self, config_dir: &Path) -> Result<()> {
        let base_dir = if config_dir.is_file()
            || config_dir
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            config_dir.parent().unwrap_or(config_dir)
        } else {
            config_dir
        };

        let prompts_subdir = base_dir.join("prompts");
        let search_dirs = [prompts_subdir.as_path(), base_dir];

        for role in [
            RoleName::Init,
            RoleName::Update,
            RoleName::Task,
            RoleName::Search,
        ] {
            if let Some(file_path) = find_role_template_file(&search_dirs, role) {
                let raw = fs::read_to_string(&file_path).with_context(|| {
                    format!("Failed to read global prompt template {:?}", file_path)
                })?;
                let tpl = parse_custom_template(
                    role,
                    &raw,
                    PromptTemplateSource::GlobalConfig(file_path),
                );
                self.templates.insert(role, tpl);
            }
        }
        Ok(())
    }

    /// Loads repository-local prompt overrides from `<repo_root>/.context-init/prompts/<role>.{md,txt}`.
    pub fn load_repo_overrides(&mut self, repo_root: &Path) -> Result<()> {
        let repo_prompts_dir = repo_root.join(".context-init").join("prompts");
        let direct_prompts_dir = repo_root.join("prompts");
        let search_dirs = [repo_prompts_dir.as_path(), direct_prompts_dir.as_path()];

        for role in [
            RoleName::Init,
            RoleName::Update,
            RoleName::Task,
            RoleName::Search,
        ] {
            if let Some(file_path) = find_role_template_file(&search_dirs, role) {
                let raw = fs::read_to_string(&file_path).with_context(|| {
                    format!("Failed to read repository prompt template {:?}", file_path)
                })?;
                let tpl =
                    parse_custom_template(role, &raw, PromptTemplateSource::RepoConfig(file_path));
                self.templates.insert(role, tpl);
            }
        }
        Ok(())
    }

    /// Renders the system and user prompts for `role` using `vars` and `self.language`.
    pub fn render(&self, role: RoleName, vars: &PromptVars) -> RenderedPrompt {
        let tpl = self.get(role);
        let lang_directive = language_instruction(&self.language);
        let schema_str = if vars.json_schema.trim().is_empty() {
            default_role_json_schema(role).to_string()
        } else {
            vars.json_schema.clone()
        };

        let system_prompt = substitute_placeholders(
            &tpl.system_template,
            &self.language,
            &lang_directive,
            role,
            vars,
            &schema_str,
        );
        let prompt = substitute_placeholders(
            &tpl.user_template,
            &self.language,
            &lang_directive,
            role,
            vars,
            &schema_str,
        );

        RenderedPrompt {
            role,
            language: self.language.clone(),
            system_prompt,
            prompt,
            source: tpl.source.clone(),
        }
    }

    /// Convenience helper to render a role template given a `facts` string.
    pub fn render_facts(&self, role: RoleName, facts: &str) -> RenderedPrompt {
        let vars = PromptVars::with_facts(facts);
        self.render(role, &vars)
    }

    /// Builds a complete `LlmRequest` for `role` from `PromptVars`, including rendered `system_prompt`,
    /// `prompt`, `facts`, and default `json_schema`.
    pub fn to_llm_request(&self, role: RoleName, vars: &PromptVars) -> LlmRequest {
        let rendered = self.render(role, vars);
        let schema = if vars.json_schema.trim().is_empty() {
            default_role_json_schema(role)
        } else {
            serde_json::from_str(&vars.json_schema)
                .unwrap_or_else(|_| default_role_json_schema(role))
        };

        let mut req = LlmRequest::new(role, rendered.prompt)
            .with_system_prompt(rendered.system_prompt)
            .with_json_schema(schema);

        if let Some(ref sf) = vars.structured_facts {
            req = req.with_facts(sf.clone());
        } else if !vars.facts.trim().is_empty() {
            req = req.with_facts(Value::String(vars.facts.clone()));
        }

        req
    }
}

/// Builds a natural-language output directive for the configured language code without hardcoding
/// a single language or model family (D17).
pub fn language_instruction(language: &str) -> String {
    let trimmed = language.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "ru" | "rus" | "russian" | "русский" => {
            "Respond and write all summaries, descriptions, and explanations in Russian (ru)."
                .to_string()
        }
        "en" | "eng" | "english" => {
            "Respond and write all summaries, descriptions, and explanations in English (en)."
                .to_string()
        }
        "de" | "deu" | "german" => {
            "Respond and write all summaries, descriptions, and explanations in German (de)."
                .to_string()
        }
        "fr" | "fra" | "french" => {
            "Respond and write all summaries, descriptions, and explanations in French (fr)."
                .to_string()
        }
        "es" | "spa" | "spanish" => {
            "Respond and write all summaries, descriptions, and explanations in Spanish (es)."
                .to_string()
        }
        "zh" | "zho" | "chinese" => {
            "Respond and write all summaries, descriptions, and explanations in Chinese (zh)."
                .to_string()
        }
        other => format!(
            "Respond and write all summaries, descriptions, and explanations in the target language '{other}'."
        ),
    }
}

fn normalize_language(raw: String) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_LANGUAGE.to_string()
    } else {
        trimmed.to_string()
    }
}

fn find_role_template_file(dirs: &[&Path], role: RoleName) -> Option<PathBuf> {
    let name = role.as_str();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        for ext in ["md", "txt"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn parse_custom_template(
    role: RoleName,
    raw_content: &str,
    source: PromptTemplateSource,
) -> PromptTemplate {
    let normalized = normalize_line_endings(raw_content).trim().to_string();
    let default_tpl = builtin_template_for_role(role);

    // Support optional `---system---` / `---user---` split in custom template files
    if let Some((sys_part, user_part)) = normalized.split_once("---user---") {
        let sys_clean = sys_part
            .trim()
            .strip_prefix("---system---")
            .unwrap_or(sys_part)
            .trim()
            .to_string();
        let user_clean = user_part.trim().to_string();
        return PromptTemplate {
            role,
            system_template: if sys_clean.is_empty() {
                default_tpl.system_template
            } else {
                sys_clean
            },
            user_template: user_clean,
            source,
        };
    }

    PromptTemplate {
        role,
        system_template: default_tpl.system_template,
        user_template: normalized,
        source,
    }
}

fn substitute_placeholders(
    template: &str,
    language: &str,
    lang_directive: &str,
    role: RoleName,
    vars: &PromptVars,
    schema_str: &str,
) -> String {
    let mut out = String::with_capacity(template.len() + vars.facts.len());
    let mut rest = template;
    while let Some(open_idx) = rest.find('{') {
        out.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx + 1..];
        if let Some(close_idx) = after_open.find('}') {
            let key = &after_open[..close_idx];
            let replacement = match key {
                "language_instruction" => Some(lang_directive),
                "language" => Some(language),
                "role" => Some(role.as_str()),
                "project" => Some(vars.project.as_str()),
                "facts" => Some(vars.facts.as_str()),
                "query" => Some(vars.query.as_str()),
                "json_schema" => Some(schema_str),
                _ => None,
            };
            if let Some(val) = replacement {
                out.push_str(val);
                rest = &after_open[close_idx + 1..];
                continue;
            }
        }
        out.push('{');
        rest = after_open;
    }
    out.push_str(rest);
    out
}

fn builtin_template_for_role(role: RoleName) -> PromptTemplate {
    match role {
        RoleName::Init => PromptTemplate {
            role: RoleName::Init,
            system_template: concat!(
                "You are a factual software architecture analyzer for role '{role}'.\n",
                "Target language code: {language}.\n",
                "{language_instruction}\n",
                "Ground every statement strictly in the provided files, entrypoints, and exported symbols. ",
                "Do not invent unverified abstractions. ",
                "Return strictly valid JSON conforming to the requested schema: ",
                "{\"subsystems\": [{\"id\": \"...\", \"name\": \"...\", \"summary\": \"...\", \"tags\": [\"...\"]}]}.",
            )
            .to_string(),
            user_template: concat!(
                "Project: {project}\n",
                "Target output language: {language}\n\n",
                "Grounded module and symbol facts:\n{facts}\n\n",
                "Generate the subsystem map JSON matching schema:\n{json_schema}",
            )
            .to_string(),
            source: PromptTemplateSource::BuiltIn,
        },
        RoleName::Update => PromptTemplate {
            role: RoleName::Update,
            system_template: concat!(
                "You are an incremental architecture card updater for role '{role}'.\n",
                "Target language code: {language}.\n",
                "{language_instruction}\n",
                "Update only the affected subsystem card based on the git diff stat, modified files, and current symbols. ",
                "Return strictly valid JSON matching the requested schema.",
            )
            .to_string(),
            user_template: concat!(
                "Project: {project}\n",
                "Target output language: {language}\n\n",
                "Incremental change facts and diff summary:\n{facts}\n\n",
                "Return the updated card JSON matching schema:\n{json_schema}",
            )
            .to_string(),
            source: PromptTemplateSource::BuiltIn,
        },
        RoleName::Task => PromptTemplate {
            role: RoleName::Task,
            system_template: concat!(
                "You are a task context synthesizer for role '{role}'.\n",
                "Target language code: {language}.\n",
                "{language_instruction}\n",
                "Synthesize a concise two-level task digest from the provided task artifacts, commit history, and modified symbols. ",
                "Return strictly valid JSON matching the requested schema.",
            )
            .to_string(),
            user_template: concat!(
                "Project: {project}\n",
                "Task context / query: {query}\n",
                "Target output language: {language}\n\n",
                "Task artifacts and repository diff facts:\n{facts}\n\n",
                "Return the task digest JSON matching schema:\n{json_schema}",
            )
            .to_string(),
            source: PromptTemplateSource::BuiltIn,
        },
        RoleName::Search => PromptTemplate {
            role: RoleName::Search,
            system_template: concat!(
                "You are a technical search query expander and candidate reranker for role '{role}'.\n",
                "Target language code: {language}.\n",
                "{language_instruction}\n",
                "Expand the search query with technical synonyms and rank the candidate subsystem/object IDs by relevance. ",
                "Return strictly valid JSON matching the requested schema.",
            )
            .to_string(),
            user_template: concat!(
                "Search query: {query}\n",
                "Target output language: {language}\n\n",
                "Candidate cards and symbols:\n{facts}\n\n",
                "Return expanded terms and ranked IDs as JSON matching schema:\n{json_schema}",
            )
            .to_string(),
            source: PromptTemplateSource::BuiltIn,
        },
    }
}
