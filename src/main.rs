use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use context_init::{generator, mcp, service, types};
use std::path::PathBuf;
use types::{ProjectAnalysis, SubsystemInfo};

#[derive(Parser, Debug)]
#[command(
    name = "context-init",
    author = "gidragir",
    version,
    about = "🚀 Инициализация и поддержание трехуровневой базы знаний (L0-L2 OpenViking) для ИИ",
    long_about = "Сканирует проект, выявляет структуру монорепозиториев, точки входа, команды сборки/тестов и генерирует высокоэффективный навигационный контекст для ИИ-ассистентов."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Путь к целевому проекту (по умолчанию ".")
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Использовать локальную Ollama для интеллектуального описания подсистем
    #[arg(short, long, default_value_t = false)]
    ai: bool,

    /// URL локального сервера Ollama
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_url: String,

    /// Модель Ollama для генерации описаний
    #[arg(short, long, default_value = "qwen2.5:14b-ctx64k")]
    model: String,

    /// Перезаписать существующую базу знаний, если она уже создана
    #[arg(short, long, default_value_t = false)]
    force: bool,

    /// Режим предварительного просмотра без записи на диск
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Глубокое сканирование исходного кода с извлечением сигнатур функций/типов
    #[arg(long, default_value_t = false)]
    deep: bool,

    /// Создать или дополнить корневой AGENTS.md ссылками на базу знаний
    #[arg(long, default_value_t = true)]
    with_agents_md: bool,

    /// Принудительно оборачивать команды в префикс `rtk` (по умолчанию автодетекция в $PATH)
    #[arg(long, default_value_t = false)]
    with_rtk: bool,

    /// Отключить оборачивание команд в префикс `rtk`
    #[arg(long, default_value_t = false)]
    no_rtk: bool,

    /// Однострочный машиночитаемый вывод без ASCII-баннеров
    #[arg(long, default_value_t = false)]
    compact: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Первичная инициализация базы знаний L0-L2
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long, default_value_t = false)]
        ai: bool,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
        #[arg(short, long, default_value = "qwen2.5:14b-ctx64k")]
        model: String,
        #[arg(short, long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        deep: bool,
        #[arg(long, default_value_t = true)]
        with_agents_md: bool,
        #[arg(long, default_value_t = false)]
        with_rtk: bool,
        #[arg(long, default_value_t = false)]
        no_rtk: bool,
        /// Однострочный машиночитаемый вывод без ASCII-баннеров
        #[arg(long, default_value_t = false)]
        compact: bool,
    },
    /// Проверить актуальность базы знаний (детектор архитектурного дрейфа)
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Завершать процесс с кодом возврата 1 при обнаружении архитектурного дрейфа
        #[arg(long, default_value_t = false)]
        exit_code: bool,
        /// Однострочный машиночитаемый вывод без ASCII-баннеров
        #[arg(long, default_value_t = false)]
        compact: bool,
    },
    /// Инкрементально обновить базу знаний (добавить новые подсистемы, обновить команды)
    Update {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long, default_value_t = false)]
        ai: bool,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
        #[arg(short, long, default_value = "qwen2.5:14b-ctx64k")]
        model: String,
        #[arg(long, default_value_t = false)]
        deep: bool,
        #[arg(long, default_value_t = true)]
        with_agents_md: bool,
        #[arg(long, default_value_t = false)]
        with_rtk: bool,
        #[arg(long, default_value_t = false)]
        no_rtk: bool,
        /// Однострочный машиночитаемый вывод без ASCII-баннеров
        #[arg(long, default_value_t = false)]
        compact: bool,
    },
    /// Упаковать файлы подсистемы в сжатый AI-контекст (встроенный AST-пакер или repomix)
    Pack {
        /// Идентификатор подсистемы из L0_index (например: core, detector, generator или all)
        subsystem_id: String,
        /// Путь к целевому проекту (по умолчанию ".")
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Выходной файл (по умолчанию вывод в stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Сжимать контекст (удалять тела функций в Rust и нормализовать whitespace) [по умолчанию: включено]
        #[arg(long, conflicts_with = "no_compress")]
        compress: bool,
        /// Отключить сжатие контекста (сохранять полный исходный код файлов)
        #[arg(long, conflicts_with = "compress")]
        no_compress: bool,
        /// Использовать внешнюю утилиту repomix вместо встроенного быстрого AST-пакера
        #[arg(long, default_value_t = false)]
        repomix: bool,
        /// Формат вывода: xml или markdown (по умолчанию xml)
        #[arg(long, default_value = "xml")]
        style: String,
    },
    /// Запустить встроенный MCP-сервер (stdio-транспорт для AI-ассистентов)
    Mcp {
        /// Путь к целевому проекту (по умолчанию ".")
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn print_banner(subtitle: &str) {
    println!(
        "{}",
        "======================================================="
            .cyan()
            .bold()
    );
    println!(
        "{} {}",
        "🗺  CONTEXT-INIT:".cyan().bold(),
        subtitle.yellow().bold()
    );
    println!(
        "{}",
        "======================================================="
            .cyan()
            .bold()
    );
}

type InitOptions<'a> = service::ServiceInitOptions<'a>;
type UpdateOptions<'a> = service::ServiceUpdateOptions<'a>;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check {
            path,
            exit_code,
            compact,
        }) => run_check(&path, compact || cli.compact, exit_code),
        Some(Commands::Pack {
            subsystem_id,
            path,
            output,
            compress: _,
            no_compress,
            repomix,
            style,
        }) => {
            let effective_compress = !no_compress;
            run_pack(
                &subsystem_id,
                &path,
                output.as_deref(),
                effective_compress,
                &style,
                repomix,
            )
        }
        Some(Commands::Update {
            path,
            ai,
            ollama_url,
            model,
            deep,
            with_agents_md,
            with_rtk,
            no_rtk,
            compact,
        }) => run_update(UpdateOptions {
            path: &path,
            ai,
            ollama_url: &ollama_url,
            model: &model,
            deep,
            with_agents_md,
            with_rtk: with_rtk || cli.with_rtk,
            no_rtk: no_rtk || cli.no_rtk,
            compact: compact || cli.compact,
        }),
        Some(Commands::Init {
            path,
            ai,
            ollama_url,
            model,
            force,
            dry_run,
            deep,
            with_agents_md,
            with_rtk,
            no_rtk,
            compact,
        }) => run_init(InitOptions {
            path: &path,
            ai,
            ollama_url: &ollama_url,
            model: &model,
            force,
            dry_run,
            deep,
            with_agents_md,
            with_rtk: with_rtk || cli.with_rtk,
            no_rtk: no_rtk || cli.no_rtk,
            compact: compact || cli.compact,
        }),
        Some(Commands::Mcp { path }) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .with_context(|| "Не удалось инициализировать асинхронный рантайм Tokio")?;
            rt.block_on(mcp::run_mcp_server(&path))
        }
        None => run_init(InitOptions {
            path: &cli.path,
            ai: cli.ai,
            ollama_url: &cli.ollama_url,
            model: &cli.model,
            force: cli.force,
            dry_run: cli.dry_run,
            deep: cli.deep,
            with_agents_md: cli.with_agents_md,
            with_rtk: cli.with_rtk,
            no_rtk: cli.no_rtk,
            compact: cli.compact,
        }),
    }
}

fn run_init(opts: InitOptions) -> Result<()> {
    if !opts.compact {
        print_banner("Инициализатор контекста L0-L2");
        print!("🔍 Анализирую проект в {:?}... ", opts.path);
    }

    let is_dry_run = opts.dry_run;
    let with_agents_md = opts.with_agents_md;
    let is_compact = opts.compact;

    let (analysis, subsystems) = service::ContextService::init_knowledge_base(opts)?;

    if !is_compact && !is_dry_run {
        println!("{}", "OK".green().bold());
        print_analysis_summary(&analysis);
    }

    if is_dry_run {
        if is_compact {
            println!(
                "OK: initialized {} subsystems in .agents/knowledge/",
                subsystems.len()
            );
            return Ok(());
        }
        println!("\n🔍 [DRY-RUN] Предварительный просмотр артефактов L0-L2:");
        println!(
            "   • L0_index.json и L0_index.md: {} подсистем",
            subsystems.len()
        );
        for sub in &subsystems {
            println!(
                "     - {} -> {} (файлов: {})",
                sub.name.yellow(),
                sub.l1_path.cyan(),
                sub.key_files.len()
            );
        }
        println!("   • cli/commands.md: актуальные команды сборки/тестирования");
        println!("   • adr/0001_initial_architecture.md: начальный архитектурный срез");
        if with_agents_md {
            println!("   • AGENTS.md: карта навигации в корне");
        }
        println!("\n✨ Файлы не были изменены (--dry-run).");
        return Ok(());
    }

    if is_compact {
        println!(
            "OK: initialized {} subsystems in .agents/knowledge/",
            subsystems.len()
        );
        return Ok(());
    }

    let target_knowledge_dir = analysis.root_dir.join(".agents").join("knowledge");
    println!("{}", "✨ УСПЕШНО!".green().bold());
    println!(
        "   База знаний инициализирована в: {}",
        target_knowledge_dir.display().to_string().cyan()
    );
    println!(
        "   • L0 Индекс:       {}",
        target_knowledge_dir.join("L0_index.md").display()
    );
    println!(
        "   • JSON Схема:      {}",
        target_knowledge_dir.join("L0_index.json").display()
    );
    println!(
        "   • Карточки L1:     {} файла(ов) в subsystems/",
        subsystems.len().to_string().yellow()
    );
    println!(
        "   • Справочник CLI:  {}",
        target_knowledge_dir.join("cli/commands.md").display()
    );
    println!(
        "   • Архитектура ADR: {}",
        target_knowledge_dir
            .join("adr/0001_initial_architecture.md")
            .display()
    );
    if with_agents_md {
        println!(
            "   • Корневой агент:  {}",
            analysis.root_dir.join("AGENTS.md").display()
        );
    }

    Ok(())
}

fn run_check(path: &std::path::Path, compact: bool, exit_code: bool) -> Result<()> {
    let has_drift = check_drift(path, compact)?;
    if exit_code && has_drift {
        std::process::exit(1);
    }
    Ok(())
}

fn check_drift(path: &std::path::Path, compact: bool) -> Result<bool> {
    let result = service::ContextService::audit_drift(path)?;
    let has_drift = result.has_drift;

    if compact {
        println!("{}", result.message);
        return Ok(has_drift);
    }

    print_banner("Аудит архитектурного дрейфа (Check)");

    print!("🔍 Анализирую состояние проекта в {:?}... ", path);
    println!("{}", "OK".green().bold());

    println!("\n📊 Результаты проверки актуальности базы знаний:");
    println!(
        "   • Актуальных подсистем: {}",
        result.up_to_date_count.to_string().green().bold()
    );

    if !result.missing_in_fs.is_empty() {
        println!(
            "   ⚠  Удалены на диске, но остались в базе знаний ({} шт.):",
            result.missing_in_fs.len().to_string().red()
        );
        for m in &result.missing_in_fs {
            println!("      - {}", m.red());
        }
    }

    if !result.untracked_in_knowledge.is_empty() {
        println!(
            "   ⚠  Новые модули на диске, отсутствующие в базе знаний ({} шт.):",
            result.untracked_in_knowledge.len().to_string().yellow()
        );
        for u in &result.untracked_in_knowledge {
            println!("      - {}", u.yellow());
        }
        println!(
            "\n💡 Запустите `context-init update`, чтобы автоматически синхронизировать новые модули."
        );
    }

    if !has_drift {
        println!(
            "{}",
            "✨ База знаний на 100% синхронизирована с файловой системой!"
                .green()
                .bold()
        );
    }

    Ok(has_drift)
}

fn run_update(opts: UpdateOptions) -> Result<()> {
    if !opts.compact {
        print_banner("Инкрементальное обновление базы знаний (Update)");
        print!("🔍 Анализирую проект в {:?}... ", opts.path);
    }

    let is_compact = opts.compact;
    let (added, updated) = service::ContextService::update_knowledge_base(opts)?;

    if is_compact {
        println!("OK: updated subsystems in .agents/knowledge/");
        return Ok(());
    }

    println!("{}", "OK".green().bold());
    println!("{}", "✨ ОБНОВЛЕНИЕ ЗАВЕРШЕНО!".green().bold());
    println!(
        "   • Добавлено новых подсистем: {}",
        added.to_string().green().bold()
    );
    println!(
        "   • Актуализировано существующих: {}",
        updated.to_string().cyan().bold()
    );

    Ok(())
}

fn print_analysis_summary(analysis: &ProjectAnalysis) {
    println!("   • Имя проекта:       {}", analysis.name.yellow().bold());
    println!(
        "   • Стек / Тип:        {}",
        analysis.project_type.display_name().magenta()
    );
    println!("   • Описание:          {}", analysis.description);
    println!(
        "   • Найдено модулей:   {}",
        format!("{} шт.", analysis.detected_modules.len()).blue()
    );
    if !analysis.workspace_members.is_empty() {
        println!(
            "   • Workspace пакеты:  {}",
            analysis.workspace_members.join(", ").blue()
        );
    }
    if !analysis.root_entrypoints.is_empty() {
        println!(
            "   • Точки входа:       {}",
            analysis.root_entrypoints.join(", ").cyan()
        );
    }
    if let Some(cmd) = &analysis.commands.test {
        println!("   • Команда тестов:    {}", cmd.green());
    }
    if let Some(cmd) = &analysis.commands.build {
        println!("   • Команда сборки:    {}", cmd.green());
    }
}

fn run_pack(
    subsystem_id: &str,
    path: &std::path::Path,
    output: Option<&std::path::Path>,
    compress: bool,
    style: &str,
    use_repomix: bool,
) -> Result<()> {
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("Не удалось получить канонический путь для {:?}", path))?;

    let (content, tokens, files_count) = service::ContextService::pack_subsystem_to_string(
        &canonical,
        subsystem_id,
        compress,
        style,
    )?;

    if use_repomix && is_command_in_path("repomix") {
        let _kdir = service::ContextService::find_knowledge_dir(&canonical)?;
        let index = service::ContextService::get_l0_index(&canonical)?;
        let matched_subs: Vec<&SubsystemInfo> = if subsystem_id.eq_ignore_ascii_case("all") {
            index.subsystems.iter().collect()
        } else {
            index
                .subsystems
                .iter()
                .filter(|s| {
                    s.id.eq_ignore_ascii_case(subsystem_id)
                        || s.name.eq_ignore_ascii_case(subsystem_id)
                        || s.id
                            .eq_ignore_ascii_case(&generator::sanitize_id(subsystem_id))
                })
                .collect()
        };

        let mut files_to_pack = Vec::new();
        for sub in &matched_subs {
            for f in &sub.files {
                if !files_to_pack.contains(&f.path) && canonical.join(&f.path).exists() {
                    files_to_pack.push(f.path.clone());
                }
            }
        }

        if !files_to_pack.is_empty() {
            let mut cmd = std::process::Command::new("repomix");
            let patterns = files_to_pack.join(",");
            cmd.arg("--include").arg(&patterns);
            if compress {
                cmd.arg("--compress");
            }
            cmd.arg("--style").arg(style);
            cmd.arg("--no-security-check");

            if let Some(out_p) = output {
                cmd.arg("-o").arg(out_p);
            } else {
                cmd.arg("-o").arg("-");
            }
            cmd.current_dir(&canonical);

            if let Ok(s) = cmd.status() {
                if s.success() {
                    if let Some(out_p) = output {
                        if out_p.to_string_lossy() != "-" {
                            eprintln!(
                                "{}",
                                format!(
                                    "✨ Подсистема '{}' успешно упакована через repomix в {:?}",
                                    subsystem_id, out_p
                                )
                                .green()
                                .bold()
                            );
                        }
                    }
                    return Ok(());
                }
            }
        }
    }

    if let Some(out_p) = output {
        std::fs::write(out_p, &content)
            .with_context(|| format!("Не удалось записать в {:?}", out_p))?;
        eprintln!(
            "{}",
            format!(
                "✨ Контекст подсистемы упакован в {:?} (~{} токенов BPE, {} файлов)",
                out_p, tokens, files_count
            )
            .green()
            .bold()
        );
    } else {
        use std::io::Write;
        std::io::stdout().write_all(content.as_bytes())?;
    }

    Ok(())
}

fn is_command_in_path(cmd: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(cmd);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use service::{compress_code, compress_file_content, pack_builtin_to_string};

    #[test]
    fn test_compress_code() {
        let raw = "fn hello() {\n\n\n    println!(\"hi\");   \n\n\n}\n";
        let compressed = compress_code(raw);
        assert_eq!(compressed, "fn hello() {\n\n    println!(\"hi\");\n\n}");
    }

    #[test]
    fn test_compress_rust_ast() {
        let code = r#"
use std::collections::HashMap;

pub struct Service {
    pub name: String,
}

impl Service {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub fn run() {
    println!("running");
}
"#;
        let compressed = compress_file_content(code, "rs");
        assert!(compressed.contains("pub fn run() {\n    ⋮----\n}"));
        assert!(compressed.contains("pub fn new(name: String) -> Self {\n        ⋮----\n    }"));
        assert!(compressed.contains("pub struct Service { pub name: String, }"));
    }

    #[test]
    fn test_compress_rust_ast_with_attributes_and_references() {
        let code = r#"
#![allow(unused)]

#[inline]
pub fn process(text: &str) -> &str {
    text
}

impl Service {
    #[inline]
    pub fn handle(&self, query: &str) -> bool {
        query.is_empty()
    }
}

#[cfg(test)]
mod tests {
    pub fn internal() {
        println!("test");
    }
}
"#;
        let compressed = compress_file_content(code, "rs");
        assert!(compressed.contains("#![allow(unused)]"));
        assert!(
            compressed.contains("#[inline]\npub fn process(text: &str) -> &str {\n    ⋮----\n}")
        );
        assert!(compressed.contains(
            "#[inline]\n    pub fn handle(&self, query: &str) -> bool {\n        ⋮----\n    }"
        ));
        assert!(compressed.contains(
            "#[cfg(test)]\nmod tests {\n    pub fn internal() {\n        ⋮----\n    }\n}"
        ));
    }

    #[test]
    fn test_pack_fallback_xml_and_md() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_sub_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack.xml");
        let out_md = temp_dir.join("pack.md");

        let xml_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, true, "xml");
        std::fs::write(&out_xml, &xml_str).unwrap();
        assert!(out_xml.exists());
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("<context_pack"));
        assert!(xml_content.contains("lib.rs"));
        assert!(xml_content.contains("⋮----"));

        let md_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, true, "markdown");
        std::fs::write(&out_md, &md_str).unwrap();
        assert!(out_md.exists());
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("# 📦 AI Context Pack:"));
        assert!(md_content.contains("## File: `lib.rs`"));
        assert!(md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_no_compress() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_nc_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack_nc.xml");
        let out_md = temp_dir.join("pack_nc.md");

        let xml_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, false, "xml");
        std::fs::write(&out_xml, &xml_str).unwrap();
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!xml_content.contains("⋮----"));

        let md_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, false, "markdown");
        std::fs::write(&out_md, &md_str).unwrap();
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_non_rust_files() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir =
            std::env::temp_dir().join(format!("test_pack_poly_{}_{}", std::process::id(), nanos));
        let _ = std::fs::create_dir_all(&temp_dir);

        let py_code = "def calculate(a, b):\n    # Indented block\n    result = a + b    \n\n\n    return result\n";
        let py_file = temp_dir.join("main.py");
        std::fs::write(&py_file, py_code).unwrap();

        let sub = SubsystemInfo {
            id: "scripts".to_string(),
            name: "Scripts".to_string(),
            summary: "Python script module".to_string(),
            l1_path: "subsystems/scripts.md".to_string(),
            tags: vec!["scripts".to_string()],
            entrypoints: vec!["main.py".to_string()],
            key_files: vec!["main.py".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 15,
        };

        let files = vec!["main.py".to_string()];
        let out_nc = temp_dir.join("pack_nc.xml");
        let out_comp = temp_dir.join("pack_comp.xml");

        // With no-compress: content is 100% identical including extra blank lines and trailing whitespace
        let nc_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, false, "xml");
        std::fs::write(&out_nc, &nc_str).unwrap();
        assert!(nc_str.contains("    result = a + b    \n\n\n    return result"));

        // With compress: trailing whitespace and excessive empty lines collapsed, but python indentation strictly kept
        let comp_str = pack_builtin_to_string(&temp_dir, &[&sub], &files, true, "xml");
        std::fs::write(&out_comp, &comp_str).unwrap();
        assert!(comp_str.contains("    result = a + b\n\n    return result"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_check_parsing() {
        // Default check: exit_code is false, compact is false
        let parsed = Cli::try_parse_from(["context-init", "check"]).unwrap();
        match parsed.command {
            Some(Commands::Check {
                path,
                exit_code,
                compact,
            }) => {
                assert_eq!(path, PathBuf::from("."));
                assert!(!exit_code);
                assert!(!compact);
            }
            _ => panic!("Expected Commands::Check"),
        }

        // check --exit-code
        let parsed = Cli::try_parse_from(["context-init", "check", "--exit-code"]).unwrap();
        match parsed.command {
            Some(Commands::Check {
                path,
                exit_code,
                compact,
            }) => {
                assert_eq!(path, PathBuf::from("."));
                assert!(exit_code);
                assert!(!compact);
            }
            _ => panic!("Expected Commands::Check"),
        }

        // check --exit-code --compact /custom/path
        let parsed = Cli::try_parse_from([
            "context-init",
            "check",
            "--exit-code",
            "--compact",
            "/custom/path",
        ])
        .unwrap();
        match parsed.command {
            Some(Commands::Check {
                path,
                exit_code,
                compact,
            }) => {
                assert_eq!(path, PathBuf::from("/custom/path"));
                assert!(exit_code);
                assert!(compact);
            }
            _ => panic!("Expected Commands::Check"),
        }
    }

    #[test]
    fn test_cli_pack_parsing() {
        // Default pack: repomix is false, no_compress is false (effective compress is true)
        let parsed = Cli::try_parse_from(["context-init", "pack", "detector"]).unwrap();
        match parsed.command {
            Some(Commands::Pack {
                subsystem_id,
                compress,
                no_compress,
                repomix,
                ..
            }) => {
                assert_eq!(subsystem_id, "detector");
                assert!(!compress);
                assert!(!no_compress);
                assert!(!repomix);
                let effective_compress = !no_compress;
                assert!(effective_compress);
            }
            _ => panic!("Expected Commands::Pack"),
        }

        // pack --no-compress
        let parsed =
            Cli::try_parse_from(["context-init", "pack", "detector", "--no-compress"]).unwrap();
        match parsed.command {
            Some(Commands::Pack {
                no_compress,
                compress,
                ..
            }) => {
                assert!(no_compress);
                assert!(!compress);
                let effective_compress = !no_compress;
                assert!(!effective_compress);
            }
            _ => panic!("Expected Commands::Pack"),
        }

        // pack --compress
        let parsed =
            Cli::try_parse_from(["context-init", "pack", "detector", "--compress"]).unwrap();
        match parsed.command {
            Some(Commands::Pack {
                compress,
                no_compress,
                ..
            }) => {
                assert!(compress);
                assert!(!no_compress);
                let effective_compress = !no_compress;
                assert!(effective_compress);
            }
            _ => panic!("Expected Commands::Pack"),
        }

        // pack --repomix
        let parsed =
            Cli::try_parse_from(["context-init", "pack", "detector", "--repomix"]).unwrap();
        match parsed.command {
            Some(Commands::Pack { repomix, .. }) => {
                assert!(repomix);
            }
            _ => panic!("Expected Commands::Pack"),
        }

        // pack with both --compress and --no-compress should fail
        let err = Cli::try_parse_from([
            "context-init",
            "pack",
            "detector",
            "--compress",
            "--no-compress",
        ]);
        assert!(err.is_err());
    }

    #[test]
    fn test_check_drift_on_clean_and_drifted_project() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir =
            std::env::temp_dir().join(format!("test_drift_{}_{}", std::process::id(), nanos));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        std::fs::write(
            temp_dir.join("Cargo.toml"),
            "[package]\nname = \"dummy\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(temp_dir.join("src").join("main.rs"), "fn main() {}").unwrap();

        // Run init first
        let opts = InitOptions {
            path: &temp_dir,
            ai: false,
            ollama_url: "http://127.0.0.1:11434",
            model: "qwen2.5:14b-ctx64k",
            force: true,
            dry_run: false,
            deep: false,
            with_agents_md: true,
            with_rtk: false,
            no_rtk: true,
            compact: true,
        };
        run_init(opts).unwrap();

        // Drift check should report false (clean)
        let has_drift = check_drift(&temp_dir, true).unwrap();
        assert!(!has_drift);

        // Add an untracked file in a new module
        let _ = std::fs::create_dir_all(temp_dir.join("src").join("extra"));
        std::fs::write(
            temp_dir.join("src").join("extra").join("mod.rs"),
            "pub fn extra() {}",
        )
        .unwrap();

        // Drift check should report true (untracked detected)
        let has_drift = check_drift(&temp_dir, true).unwrap();
        assert!(has_drift);

        // Remove extra module
        let _ = std::fs::remove_dir_all(temp_dir.join("src").join("extra"));
        assert!(!check_drift(&temp_dir, true).unwrap());

        // Now test missing_in_fs: remove src directory entirely so tracked subsystem is missing
        let _ = std::fs::remove_dir_all(temp_dir.join("src"));
        let has_drift_missing = check_drift(&temp_dir, true).unwrap();
        assert!(has_drift_missing);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_version_matches_cargo_pkg_version_and_workspace_crates() {
        use clap::CommandFactory;
        let cmd = Cli::command();
        assert_eq!(cmd.get_version(), Some(env!("CARGO_PKG_VERSION")));
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.3.0");
        assert_eq!(ci_core::VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(ci_llm::VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(ci_mcp::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
