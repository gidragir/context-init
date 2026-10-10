use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router, ServiceExt};

use crate::service::{ContextService, ServiceInitOptions, ServiceUpdateOptions};
use crate::types::{
    McpCheckParams, McpCheckResult, McpGetSubsystemsParams, McpInitParams, McpPackParams,
    McpUpdateParams, SubsystemInfo,
};

/// MCP Server для context-init
#[derive(Clone)]
pub struct ContextInitServer {
    default_path: Arc<PathBuf>,
}

impl ContextInitServer {
    pub fn new(default_path: PathBuf) -> Self {
        Self {
            default_path: Arc::new(default_path),
        }
    }

    fn resolve_path(&self, requested: Option<&str>) -> PathBuf {
        match requested {
            Some(p) if !p.trim().is_empty() => PathBuf::from(p),
            _ => (*self.default_path).clone(),
        }
    }
}

#[tool_router(server_handler)]
impl ContextInitServer {
    /// Проверить актуальность базы знаний (детектор архитектурного дрейфа)
    #[tool(
        name = "context_check",
        description = "Проверяет актуальность базы знаний OpenViking L0-L2 относительно файлов на диске. Возвращает статус дрейфа, список удаленных и новых файлов."
    )]
    pub async fn check_drift(
        &self,
        params: Parameters<McpCheckParams>,
    ) -> Result<Json<McpCheckResult>, rmcp::ErrorData> {
        let path = self.resolve_path(params.0.path.as_deref());
        match ContextService::audit_drift(&path) {
            Ok(res) => Ok(Json(res)),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("Ошибка проверки архитектурного дрейфа: {:#}", e),
                None,
            )),
        }
    }

    /// Упаковать файлы подсистемы в сжатый AI-контекст (AST-вырезание тел функций)
    #[tool(
        name = "context_pack",
        description = "Упаковывает файлы подсистемы в AST-сжатый AI-контекст (вырезает тела функций до ⋮---- в Rust и нормализует пробелы). Позволяет исследовать код подсистемы без раздувания контекста."
    )]
    pub async fn pack(&self, params: Parameters<McpPackParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let path = self.resolve_path(p.path.as_deref());
        let compress = p.compress.unwrap_or(true);
        let style = p.style.unwrap_or_else(|| "xml".to_string());

        match ContextService::pack_subsystem_to_string(&path, &p.subsystem_id, compress, &style) {
            Ok((content, tokens, files_count)) => {
                let header = format!(
                    "<!-- context-init pack: subsystem='{}', files={}, tokens~{} -->\n",
                    p.subsystem_id, files_count, tokens
                );
                Ok(format!("{}{}", header, content))
            }
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("Ошибка упаковки подсистемы '{}': {:#}", p.subsystem_id, e),
                None,
            )),
        }
    }

    /// Получить список всех подсистем проекта и их метрики из L0 индекса
    #[tool(
        name = "context_get_subsystems",
        description = "Возвращает структурированный список всех подсистем проекта из L0_index.json с точками входа, ключевыми файлами и количеством токенов."
    )]
    pub async fn get_subsystems(
        &self,
        params: Parameters<McpGetSubsystemsParams>,
    ) -> Result<Json<Vec<SubsystemInfo>>, rmcp::ErrorData> {
        let path = self.resolve_path(params.0.path.as_deref());
        match ContextService::get_subsystems(&path) {
            Ok(subs) => Ok(Json(subs)),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("Ошибка чтения подсистем: {:#}", e),
                None,
            )),
        }
    }

    /// Инкрементально обновить базу знаний OpenViking (синхронизировать новые модули)
    #[tool(
        name = "context_update",
        description = "Инкрементально обновляет базу знаний OpenViking при обнаружении архитектурного дрейфа без затирания ручных заметок."
    )]
    pub async fn update(
        &self,
        params: Parameters<McpUpdateParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let path = self.resolve_path(p.path.as_deref());
        let deep = p.deep.unwrap_or(false);
        let ai = p.ai.unwrap_or(false);
        let model = p.model.unwrap_or_else(|| "qwen2.5:14b-ctx64k".to_string());

        let opts = ServiceUpdateOptions {
            path: &path,
            ai,
            ollama_url: "http://127.0.0.1:11434",
            model: &model,
            deep,
            with_agents_md: true,
            with_rtk: false,
            no_rtk: true,
            compact: true,
        };

        match ContextService::update_knowledge_base(opts) {
            Ok((added, updated)) => Ok(format!(
                "Синхронизация завершена: добавлено {} новых подсистем, актуализировано {} существующих.",
                added, updated
            )),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("Ошибка обновления базы знаний: {:#}", e),
                None,
            )),
        }
    }

    /// Первичная инициализация трехуровневой базы знаний OpenViking L0-L2
    #[tool(
        name = "context_init",
        description = "Выполняет первичную инициализацию базы знаний OpenViking L0-L2 (.agents/knowledge/) для проекта."
    )]
    pub async fn init(&self, params: Parameters<McpInitParams>) -> Result<String, rmcp::ErrorData> {
        let p = params.0;
        let path = self.resolve_path(p.path.as_deref());
        let deep = p.deep.unwrap_or(false);
        let ai = p.ai.unwrap_or(false);
        let force = p.force.unwrap_or(false);
        let with_agents_md = p.with_agents_md.unwrap_or(true);

        let opts = ServiceInitOptions {
            path: &path,
            ai,
            ollama_url: "http://127.0.0.1:11434",
            model: "qwen2.5:14b-ctx64k",
            force,
            dry_run: false,
            deep,
            with_agents_md,
            with_rtk: false,
            no_rtk: true,
            compact: true,
        };

        match ContextService::init_knowledge_base(opts) {
            Ok((analysis, subs)) => Ok(format!(
                "База знаний успешно инициализирована для проекта '{}' ({} подсистем).",
                analysis.name,
                subs.len()
            )),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("Ошибка инициализации базы знаний: {:#}", e),
                None,
            )),
        }
    }
}

/// Запустить MCP-сервер через stdio-транспорт
pub async fn run_mcp_server(path: &Path) -> anyhow::Result<()> {
    // Внимание: все диагностические сообщения должны идти строго в stderr!
    eprintln!(
        "🚀 Запуск context-init MCP сервера (stdio) для рабочей директории {:?}",
        path
    );

    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };

    let server = ContextInitServer::new(canonical);
    let (stdin, stdout) = rmcp::transport::io::stdio();

    // Запускаем обслуживание MCP по протоколу stdio
    let running = server
        .serve((stdin, stdout))
        .await
        .map_err(|e| anyhow::anyhow!("Ошибка инициализации MCP сервера: {:?}", e))?;

    eprintln!("✨ context-init MCP сервер готов принимать запросы.");
    let _ = running.waiting().await;
    eprintln!("👋 context-init MCP сервер остановлен.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_mcp_check_and_pack_on_dummy_project() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir =
            std::env::temp_dir().join(format!("test_mcp_{}_{}", std::process::id(), nanos));
        let src_dir = temp_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();

        fs::write(
            temp_dir.join("Cargo.toml"),
            "[package]\nname = \"dummy-mcp\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();

        let server = ContextInitServer::new(temp_dir.clone());

        // 1. context_init
        let init_res = server
            .init(Parameters(McpInitParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
                deep: Some(false),
                ai: Some(false),
                force: Some(true),
                with_agents_md: Some(true),
            }))
            .await;
        assert!(init_res.is_ok(), "init failed: {:?}", init_res.err());

        // 2. context_check (should be clean)
        let check_res = server
            .check_drift(Parameters(McpCheckParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
            }))
            .await;
        assert!(check_res.is_ok(), "check failed: {:?}", check_res.err());
        let check_data = check_res.unwrap().0;
        assert!(!check_data.has_drift);
        assert_eq!(check_data.missing_in_fs.len(), 0);
        assert_eq!(check_data.untracked_in_knowledge.len(), 0);

        // 3. context_get_subsystems
        let subs_res = server
            .get_subsystems(Parameters(McpGetSubsystemsParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
            }))
            .await;
        assert!(
            subs_res.is_ok(),
            "get_subsystems failed: {:?}",
            subs_res.err()
        );
        let subs_list = subs_res.unwrap().0;
        assert!(!subs_list.is_empty());
        let first_id = &subs_list[0].id;

        // 4. context_pack
        let pack_res = server
            .pack(Parameters(McpPackParams {
                subsystem_id: first_id.clone(),
                path: Some(temp_dir.to_string_lossy().to_string()),
                compress: Some(true),
                style: Some("xml".to_string()),
            }))
            .await;
        assert!(pack_res.is_ok(), "pack failed: {:?}", pack_res.err());
        let pack_str = pack_res.unwrap();
        assert!(pack_str.contains("context_pack"));
        assert!(pack_str.contains("main.rs"));

        // 5. Test drift detection: add a new subsystem module
        let extra_dir = src_dir.join("extra");
        fs::create_dir_all(&extra_dir).unwrap();
        fs::write(extra_dir.join("mod.rs"), "pub fn helper() {}\n").unwrap();

        let check_drifted = server
            .check_drift(Parameters(McpCheckParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
            }))
            .await;
        assert!(check_drifted.is_ok());
        let drift_data = check_drifted.unwrap().0;
        assert!(
            drift_data.has_drift,
            "Should detect architectural drift when new module is added"
        );
        assert_eq!(drift_data.untracked_in_knowledge.len(), 1);

        // 6. Test context_update tool to reconcile drift
        let update_res = server
            .update(Parameters(McpUpdateParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
                deep: Some(false),
                ai: Some(false),
                model: None,
            }))
            .await;
        assert!(update_res.is_ok(), "update failed: {:?}", update_res.err());

        // 7. Re-check drift after update (must be clean)
        let check_reconciled = server
            .check_drift(Parameters(McpCheckParams {
                path: Some(temp_dir.to_string_lossy().to_string()),
            }))
            .await;
        assert!(check_reconciled.is_ok());
        let reconciled_data = check_reconciled.unwrap().0;
        assert!(
            !reconciled_data.has_drift,
            "Knowledge base must be clean after update"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
