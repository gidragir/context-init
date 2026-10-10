mod detector;
mod generator;
mod mcp;
mod ollama;
mod service;
mod types;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use types::{ProjectAnalysis, SubsystemInfo};

#[derive(Parser, Debug)]
#[command(
    name = "context-init",
    author = "gidragir",
    version = "0.2.0",
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

struct InitOptions<'a> {
    pub path: &'a std::path::Path,
    pub ai: bool,
    pub ollama_url: &'a str,
    pub model: &'a str,
    pub force: bool,
    pub dry_run: bool,
    pub deep: bool,
    pub with_agents_md: bool,
    pub with_rtk: bool,
    pub no_rtk: bool,
    pub compact: bool,
}

struct UpdateOptions<'a> {
    pub path: &'a std::path::Path,
    pub ai: bool,
    pub ollama_url: &'a str,
    pub model: &'a str,
    pub deep: bool,
    pub with_agents_md: bool,
    pub with_rtk: bool,
    pub no_rtk: bool,
    pub compact: bool,
}

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
    let use_rtk = if opts.no_rtk {
        false
    } else {
        opts.with_rtk || detector::is_rtk_available()
    };

    if !opts.compact {
        print_banner("Инициализатор контекста L0-L2");
        print!("🔍 Анализирую проект в {:?}... ", opts.path);
    }
    let analysis = detector::analyze_project(opts.path, opts.deep, use_rtk)?;
    if !opts.compact {
        println!("{}", "OK".green().bold());
        print_analysis_summary(&analysis);
    }

    let subsystems = build_subsystems(
        &analysis,
        opts.ai,
        opts.ollama_url,
        opts.model,
        opts.compact,
    )?;

    if opts.dry_run {
        if opts.compact {
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
        if opts.with_agents_md {
            println!("   • AGENTS.md: карта навигации в корне");
        }
        println!("\n✨ Файлы не были изменены (--dry-run).");
        return Ok(());
    }

    if !opts.compact {
        println!("\n📦 Генерирую файловую структуру L0-L2 в .agents/knowledge/...");
    }
    generator::generate_knowledge_base(&analysis, &subsystems, opts.force, opts.with_agents_md)?;

    if opts.compact {
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
    if opts.with_agents_md {
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
    let analysis = detector::analyze_project(path, false, false)?;
    let report = generator::audit_knowledge_base(&analysis)?;
    let has_drift = !report.missing_in_fs.is_empty() || !report.untracked_in_knowledge.is_empty();

    if compact {
        if !has_drift {
            println!("OK: {} subsystems up-to-date", report.up_to_date_count);
        } else {
            println!(
                "DRIFT: up_to_date={}, missing=[{}], untracked=[{}]",
                report.up_to_date_count,
                report.missing_in_fs.join(", "),
                report.untracked_in_knowledge.join(", ")
            );
        }
        return Ok(has_drift);
    }

    print_banner("Аудит архитектурного дрейфа (Check)");

    print!("🔍 Анализирую состояние проекта в {:?}... ", path);
    println!("{}", "OK".green().bold());

    println!("\n📊 Результаты проверки актуальности базы знаний:");
    println!(
        "   • Актуальных подсистем: {}",
        report.up_to_date_count.to_string().green().bold()
    );

    if !report.missing_in_fs.is_empty() {
        println!(
            "   ⚠  Удалены на диске, но остались в базе знаний ({} шт.):",
            report.missing_in_fs.len().to_string().red()
        );
        for m in &report.missing_in_fs {
            println!("      - {}", m.red());
        }
    }

    if !report.untracked_in_knowledge.is_empty() {
        println!(
            "   ⚠  Новые модули на диске, отсутствующие в базе знаний ({} шт.):",
            report.untracked_in_knowledge.len().to_string().yellow()
        );
        for u in &report.untracked_in_knowledge {
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
    let use_rtk = if opts.no_rtk {
        false
    } else {
        opts.with_rtk || detector::is_rtk_available()
    };

    if !opts.compact {
        print_banner("Инкрементальное обновление базы знаний (Update)");
        print!("🔍 Анализирую проект в {:?}... ", opts.path);
    }
    let analysis = detector::analyze_project(opts.path, opts.deep, use_rtk)?;
    if !opts.compact {
        println!("{}", "OK".green().bold());
    }

    let subsystems = build_subsystems(
        &analysis,
        opts.ai,
        opts.ollama_url,
        opts.model,
        opts.compact,
    )?;

    let (added, updated) =
        generator::update_knowledge_base(&analysis, &subsystems, opts.with_agents_md)?;

    if opts.compact {
        println!(
            "OK: updated {} subsystems in .agents/knowledge/",
            subsystems.len()
        );
        return Ok(());
    }

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

fn build_subsystems(
    analysis: &ProjectAnalysis,
    ai: bool,
    ollama_url: &str,
    model: &str,
    compact: bool,
) -> Result<Vec<SubsystemInfo>> {
    let mut subsystems: Vec<SubsystemInfo> = Vec::new();

    if ai {
        if !compact {
            println!(
                "\n🤖 Режим ИИ активирован: опрашиваю Ollama ({}, модель {})...",
                ollama_url, model
            );
        }
        if ollama::is_ollama_online(ollama_url) {
            match ollama::enrich_subsystems_with_ai(analysis, ollama_url, model) {
                Ok(ai_subs) if !ai_subs.is_empty() => {
                    if !compact {
                        println!(
                            "   ✓ Получено описание для {} подсистем от ИИ.",
                            ai_subs.len().to_string().green().bold()
                        );
                    }
                    for sub in ai_subs {
                        let matching_mod = analysis.detected_modules.iter().find(|m| {
                            let clean = generator::sanitize_id(&m.dir_path);
                            clean == sub.id
                                || clean == format!("src_{}", sub.id)
                                || m.name == sub.name
                                || m.name == sub.id
                                || (sub.id == "core" && (m.name == "main" || m.name == "core"))
                        });

                        let entrypoints = matching_mod
                            .map(|m| m.entrypoints.clone())
                            .unwrap_or_default();
                        let key_files = matching_mod
                            .map(|m| m.files.iter().map(|f| f.path.clone()).collect())
                            .unwrap_or_default();
                        let files = matching_mod.map(|m| m.files.clone()).unwrap_or_default();
                        let key_symbols =
                            matching_mod.map(|m| m.symbols.clone()).unwrap_or_default();
                        let test_cmd = matching_mod.and_then(|m| m.test_cmd.clone());

                        subsystems.push(SubsystemInfo {
                            l1_path: format!("subsystems/{}.md", sub.id),
                            id: sub.id,
                            name: sub.name,
                            summary: sub.summary,
                            tags: sub.tags,
                            entrypoints,
                            key_files,
                            files,
                            key_symbols,
                            test_command: test_cmd,
                            tokens: 0,
                        });
                    }
                }
                Err(err) if !compact => {
                    println!(
                        "   ⚠ Запрос к Ollama завершился с ошибкой: {}. Переход на эвристический анализ.",
                        err
                    );
                }
                _ => {}
            }
        } else if !compact {
            println!(
                "   ⚠ Ollama недоступна на {}. Использую эвристический анализ.",
                ollama_url
            );
        }
    }

    if subsystems.is_empty() {
        if analysis.detected_modules.is_empty() {
            // Плоский проект
            subsystems.push(SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: format!("Основная логика и компоненты проекта {}", analysis.name),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["core".to_string(), "main".to_string()],
                entrypoints: analysis.root_entrypoints.clone(),
                key_files: analysis.root_entrypoints.clone(),
                files: Vec::new(),
                key_symbols: Vec::new(),
                test_command: analysis.commands.test.clone(),
                tokens: 0,
            });
        } else {
            for m in &analysis.detected_modules {
                let clean_id = generator::sanitize_id(&m.dir_path);
                let capitalized = m
                    .name
                    .replace(['_', '-'], " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                let summary = if !m.entrypoints.is_empty() {
                    format!(
                        "Подсистема `{}` с ключевыми точками входа: {}",
                        m.dir_path,
                        m.entrypoints.join(", ")
                    )
                } else {
                    format!(
                        "Модуль и компоненты директории `{}` ({} файлов)",
                        m.dir_path,
                        m.files.len()
                    )
                };

                let key_files = m.files.iter().map(|f| f.path.clone()).collect();

                subsystems.push(SubsystemInfo {
                    id: clean_id.clone(),
                    name: capitalized,
                    summary,
                    l1_path: format!("subsystems/{}.md", clean_id),
                    tags: vec![clean_id, analysis.project_type.display_name().to_string()],
                    entrypoints: m.entrypoints.clone(),
                    key_files,
                    files: m.files.clone(),
                    key_symbols: m.symbols.clone(),
                    test_command: m.test_cmd.clone(),
                    tokens: 0,
                });
            }
        }
    }

    for sub in &mut subsystems {
        compute_subsystem_tokens(&analysis.root_dir, sub);
    }

    Ok(subsystems)
}

fn compute_subsystem_tokens(root_dir: &std::path::Path, sub: &mut SubsystemInfo) {
    let mut total_tokens = 0;
    if !sub.files.is_empty() {
        for f in &sub.files {
            let path = root_dir.join(&f.path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    } else {
        for path_str in &sub.key_files {
            let path = root_dir.join(path_str);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    }
    sub.tokens = total_tokens;
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

    let kdir = service::ContextService::find_knowledge_dir(&canonical)?;
    let l0_json_path = kdir.join("L0_index.json");
    if !l0_json_path.exists() {
        anyhow::bail!(
            "База знаний не найдена по пути {:?}. Сначала выполните `context-init init`.",
            l0_json_path
        );
    }

    let content = std::fs::read_to_string(&l0_json_path)
        .with_context(|| format!("Не удалось прочитать {:?}", l0_json_path))?;
    let index: types::L0Index = serde_json::from_str(&content)
        .with_context(|| format!("Ошибка десериализации L0_index.json в {:?}", l0_json_path))?;

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

    if matched_subs.is_empty() {
        let available: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
        anyhow::bail!(
            "Подсистема '{}' не найдена в L0_index.json. Доступные подсистемы: {}",
            subsystem_id,
            available.join(", ")
        );
    }

    let mut files_to_pack = Vec::new();
    for sub in &matched_subs {
        if !sub.files.is_empty() {
            for f in &sub.files {
                if !files_to_pack.contains(&f.path) && canonical.join(&f.path).exists() {
                    files_to_pack.push(f.path.clone());
                }
            }
        } else if !sub.key_files.is_empty() {
            for kf in &sub.key_files {
                if !files_to_pack.contains(kf) && canonical.join(kf).exists() {
                    files_to_pack.push(kf.clone());
                }
            }
        } else {
            let l1_file = kdir.join(&sub.l1_path);
            if l1_file.exists() {
                let rel = l1_file
                    .strip_prefix(&canonical)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| sub.l1_path.clone());
                if !files_to_pack.contains(&rel) {
                    files_to_pack.push(rel);
                }
            }
        }
    }

    if files_to_pack.is_empty() {
        anyhow::bail!(
            "Для подсистемы '{}' не найдено существующих файлов на диске.",
            subsystem_id
        );
    }

    if use_repomix {
        if is_command_in_path("repomix") {
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

            let status = cmd.status();
            match status {
                Ok(s) if s.success() => {
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
                _ => {
                    eprintln!(
                        "{}",
                        "⚠ repomix завершился с ошибкой, переключаюсь на встроенный AST-пакер..."
                            .yellow()
                    );
                }
            }
        } else {
            eprintln!(
                "{}",
                "⚠ Утилита repomix не найдена в $PATH, переключаюсь на встроенный AST-пакер..."
                    .yellow()
            );
        }
    }

    pack_builtin(
        &canonical,
        &matched_subs,
        &files_to_pack,
        output,
        compress,
        style,
    )?;

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

fn pack_builtin(
    root: &std::path::Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    output: Option<&std::path::Path>,
    compress: bool,
    style: &str,
) -> Result<()> {
    let mut buffer = String::new();

    if style.eq_ignore_ascii_case("markdown") {
        buffer.push_str("# 📦 AI Context Pack: ");
        let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
        buffer.push_str(&names.join(", "));
        buffer.push_str("\n\n> Сгенерировано context-init (встроенный AST-пакер)\n\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = std::fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_file_content(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "## File: `{}`\n```{}\n{}\n```\n\n",
                    file_rel, ext, processed
                ));
            }
        }
    } else {
        buffer.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        buffer.push_str("<context_pack generated_by=\"context-init\">\n");
        for sub in subs {
            buffer.push_str(&format!(
                "  <subsystem id=\"{}\" name=\"{}\">\n    <summary>{}</summary>\n  </subsystem>\n",
                sub.id, sub.name, sub.summary
            ));
        }
        buffer.push_str("  <files>\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = std::fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_file_content(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "    <file path=\"{}\">\n<![CDATA[\n{}\n]]>\n    </file>\n",
                    file_rel, processed
                ));
            }
        }
        buffer.push_str("  </files>\n</context_pack>\n");
    }

    let tokens = detector::count_tokens(&buffer);

    if let Some(out_p) = output {
        std::fs::write(out_p, &buffer)
            .with_context(|| format!("Не удалось записать в {:?}", out_p))?;
        eprintln!(
            "{}",
            format!(
                "✨ Контекст подсистемы упакован в {:?} (~{} токенов BPE, {} файлов)",
                out_p,
                tokens,
                files.len()
            )
            .green()
            .bold()
        );
    } else {
        use std::io::Write;
        std::io::stdout().write_all(buffer.as_bytes())?;
    }

    Ok(())
}

#[allow(dead_code)]
fn pack_fallback(
    root: &std::path::Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    output: Option<&std::path::Path>,
    compress: bool,
    style: &str,
) -> Result<()> {
    pack_builtin(root, subs, files, output, compress, style)
}

fn compress_file_content(content: &str, ext: &str) -> String {
    if ext.eq_ignore_ascii_case("rs") {
        if let Some(compressed) = compress_rust_ast(content) {
            return compressed;
        }
    }
    compress_code(content)
}

fn compress_rust_ast(content: &str) -> Option<String> {
    let parsed = syn::parse_file(content).ok()?;
    let mut file_attrs = Vec::new();
    for attr in &parsed.attrs {
        file_attrs.push(normalize_rust_tokens(&quote::quote!(#attr).to_string()));
    }
    let body = format_rust_items(&parsed.items);
    if file_attrs.is_empty() {
        Some(body)
    } else {
        Some(format!("{}\n\n{}", file_attrs.join("\n"), body))
    }
}

fn format_rust_items(items: &[syn::Item]) -> String {
    let mut formatted_items = Vec::new();
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                let attrs = &f.attrs;
                let vis = &f.vis;
                let sig = &f.sig;
                let sig_str = quote::quote!(#vis #sig).to_string();
                let clean_sig = normalize_rust_tokens(&sig_str);
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                if attr_strs.is_empty() {
                    formatted_items.push(format!("{} {{\n    ⋮----\n}}", clean_sig));
                } else {
                    formatted_items.push(format!(
                        "{}\n{} {{\n    ⋮----\n}}",
                        attr_strs.join("\n"),
                        clean_sig
                    ));
                }
            }
            syn::Item::Impl(imp) => {
                let attrs = &imp.attrs;
                let defaultness = &imp.defaultness;
                let unsafety = &imp.unsafety;
                let generics = &imp.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let self_ty = &imp.self_ty;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw = if let Some((bang, path, for_tok)) = &imp.trait_ {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #bang #path #for_tok #self_ty #where_clause).to_string()
                } else {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #self_ty #where_clause)
                        .to_string()
                };
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut item_strs = Vec::new();
                for it in &imp.items {
                    if let syn::ImplItem::Fn(m) = it {
                        let m_attrs: Vec<String> = m
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let vis = &m.vis;
                        let defaultness = &m.defaultness;
                        let sig = &m.sig;
                        let sig_str = quote::quote!(#vis #defaultness #sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        if m_attrs.is_empty() {
                            item_strs.push(format!("    {} {{\n        ⋮----\n    }}", clean_sig));
                        } else {
                            item_strs.push(format!(
                                "{}\n    {} {{\n        ⋮----\n    }}",
                                m_attrs.join("\n"),
                                clean_sig
                            ));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        item_strs.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if item_strs.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        item_strs.join("\n\n")
                    ));
                }
            }
            syn::Item::Trait(t) => {
                let attrs = &t.attrs;
                let vis = &t.vis;
                let ident = &t.ident;
                let generics = &t.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw =
                    quote::quote!(#vis trait #ident #impl_generics #where_clause).to_string();
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut trait_items = Vec::new();
                for it in &t.items {
                    if let syn::TraitItem::Fn(tf) = it {
                        let tf_attrs: Vec<String> = tf
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let sig = &tf.sig;
                        let sig_str = quote::quote!(#sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        let body = if tf.default.is_some() {
                            format!("    {} {{\n        ⋮----\n    }}", clean_sig)
                        } else {
                            format!("    {};", clean_sig)
                        };
                        if tf_attrs.is_empty() {
                            trait_items.push(body);
                        } else {
                            trait_items.push(format!("{}\n{}", tf_attrs.join("\n"), body));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        trait_items.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if trait_items.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        trait_items.join("\n\n")
                    ));
                }
            }
            syn::Item::Mod(m) => {
                let attrs = &m.attrs;
                let vis = &m.vis;
                let ident = &m.ident;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let mod_header_raw = quote::quote!(#vis mod #ident).to_string();
                let clean_header_core = normalize_rust_tokens(&mod_header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                if let Some((_, inner_items)) = &m.content {
                    let inner = format_rust_items(inner_items);
                    let indented: Vec<String> = inner
                        .lines()
                        .map(|l| {
                            if l.is_empty() {
                                String::new()
                            } else {
                                format!("    {}", l)
                            }
                        })
                        .collect();
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        indented.join("\n")
                    ));
                } else {
                    formatted_items.push(format!("{};", clean_header));
                }
            }
            other => {
                let s = quote::quote!(#other).to_string();
                formatted_items.push(normalize_rust_tokens(&s));
            }
        }
    }
    formatted_items.join("\n\n")
}

fn normalize_rust_tokens(s: &str) -> String {
    let s = s
        .replace("# ! [ ", "#![")
        .replace("# ! [", "#![")
        .replace("#![ ", "#![")
        .replace("# ! ", "#!")
        .replace("# !", "#!")
        .replace("# [ ", "#[")
        .replace("# [", "#[")
        .replace("#[ ", "#[")
        .replace(" < ", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" > ", "> ")
        .replace(" ( ", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" : ", ": ")
        .replace(" , ", ", ")
        .replace(" ;", ";")
        .replace(" :: ", "::")
        .replace("( &", "(&")
        .replace("(& ", "(&")
        .replace("& mut ", "&mut ")
        .replace("& '", "&'")
        .replace("& ", "&");

    let words: Vec<&str> = s.split_whitespace().collect();
    let joined = words.join(" ");
    joined
        .replace(" (", "(")
        .replace(":: ", "::")
        .replace(" ::", "::")
        .replace(":(", ": (")
        .replace("->(", "-> (")
        .replace("+(", "+ (")
}

fn compress_code(content: &str) -> String {
    let mut result = Vec::new();
    let mut prev_was_empty = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_empty {
                result.push("");
                prev_was_empty = true;
            }
        } else {
            prev_was_empty = false;
            result.push(line.trim_end());
        }
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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

        pack_fallback(&temp_dir, &[&sub], &files, Some(&out_xml), true, "xml").unwrap();
        assert!(out_xml.exists());
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("<context_pack"));
        assert!(xml_content.contains("lib.rs"));
        assert!(xml_content.contains("⋮----"));

        pack_fallback(&temp_dir, &[&sub], &files, Some(&out_md), true, "markdown").unwrap();
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

        pack_builtin(&temp_dir, &[&sub], &files, Some(&out_xml), false, "xml").unwrap();
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!xml_content.contains("⋮----"));

        pack_builtin(&temp_dir, &[&sub], &files, Some(&out_md), false, "markdown").unwrap();
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
        pack_builtin(&temp_dir, &[&sub], &files, Some(&out_nc), false, "xml").unwrap();
        let nc_str = std::fs::read_to_string(&out_nc).unwrap();
        assert!(nc_str.contains("    result = a + b    \n\n\n    return result"));

        // With compress: trailing whitespace and excessive empty lines collapsed, but python indentation strictly kept
        pack_builtin(&temp_dir, &[&sub], &files, Some(&out_comp), true, "xml").unwrap();
        let comp_str = std::fs::read_to_string(&out_comp).unwrap();
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
}
