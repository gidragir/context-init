mod detector;
mod generator;
mod ollama;
mod types;

use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::PathBuf;
use types::SubsystemInfo;

#[derive(Parser, Debug)]
#[command(
    name = "context-init",
    author = "gidragir",
    version = "0.1.0",
    about = "🚀 Инициализация трехуровневой базы знаний (L0-L2 OpenViking) для любого проекта",
    long_about = "Сканирует проект, определяет его тип (Rust, TypeScript, Python, Go, DevOps), выявляет модули и создает структуру .agents/knowledge/ для быстрой работы ИИ-ассистентов."
)]
struct Args {
    /// Путь к целевому проекту (по умолчанию текущая директория)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Использовать локальную Ollama для интеллектуального описания подсистем
    #[arg(short, long, default_value_t = false)]
    ai: bool,

    /// URL локального сервера Ollama
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_url: String,

    /// Модель Ollama для генерации описаний
    #[arg(short, long, default_value = "qwen2.5-coder:14b")]
    model: String,

    /// Перезаписать существующую базу знаний, если она уже создана
    #[arg(short, long, default_value_t = false)]
    force: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("{}", "=======================================================".cyan().bold());
    println!("{}", "🗺  CONTEXT-INIT: Инициализатор контекста L0-L2 (OpenViking)".cyan().bold());
    println!("{}", "=======================================================".cyan().bold());

    // 1. Анализ структуры проекта
    print!("🔍 Анализирую проект в {:?}... ", args.path);
    let analysis = detector::analyze_project(&args.path)?;
    println!("{}", "OK".green().bold());

    println!("   • Имя проекта:    {}", analysis.name.yellow().bold());
    println!("   • Тип проекта:    {}", analysis.project_type.display_name().magenta());
    println!("   • Описание:       {}", analysis.description);
    println!("   • Найдено папок:  {}", format!("{} шт.", analysis.detected_modules.len()).blue());

    // 2. Формирование подсистем (с ИИ или эвристически)
    let mut subsystems: Vec<SubsystemInfo> = Vec::new();

    if args.ai {
        println!("\n🤖 Режим ИИ активирован: проверяю доступность Ollama...");
        if ollama::is_ollama_online(&args.ollama_url) {
            println!("   ✓ Ollama доступна ({}), отправляю запрос к {}...", args.ollama_url, args.model);
            match ollama::enrich_subsystems_with_ai(&analysis, &args.ollama_url, &args.model) {
                Ok(ai_subs) if !ai_subs.is_empty() => {
                    println!("   ✓ ИИ успешно сгенерировал описание для {} подсистем.", ai_subs.len().to_string().green().bold());
                    for sub in ai_subs {
                        subsystems.push(SubsystemInfo {
                            l1_path: format!("subsystems/{}.md", sub.id),
                            id: sub.id,
                            name: sub.name,
                            summary: sub.summary,
                            tags: sub.tags,
                        });
                    }
                }
                Err(err) => {
                    println!("   ⚠ Не удалось получить ответ от ИИ: {}. Перехожу на эвристику.", err);
                }
                _ => {}
            }
        } else {
            println!("   ⚠ Ollama недоступна на {}. Использую эвристическое сканирование.", args.ollama_url);
        }
    }

    // Если ИИ не использовался или вернул пустой результат — строим подсистемы по папкам
    if subsystems.is_empty() {
        if analysis.detected_modules.is_empty() {
            // Если проект плоский (нет явных папок), создаем базовую подсистему core
            subsystems.push(SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: format!("Основная логика и компоненты проекта {}", analysis.name),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["core".to_string(), "main".to_string()],
            });
        } else {
            for dir in &analysis.detected_modules {
                let clean_id = dir.replace('/', "_").replace('\\', "_").to_lowercase();
                let clean_name = dir.split('/').last().unwrap_or(dir).replace('_', " ");
                let capitalized = clean_name
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

                subsystems.push(SubsystemInfo {
                    id: clean_id.clone(),
                    name: capitalized,
                    summary: format!("Модуль и компоненты директории {}", dir),
                    l1_path: format!("subsystems/{}.md", clean_id),
                    tags: vec![clean_id],
                });
            }
        }
    }

    // 3. Генерация файлов
    println!("\n📦 Генерирую файловую структуру L0-L2 в .agents/knowledge/...");
    generator::generate_knowledge_base(&analysis, subsystems.clone(), args.force)?;

    let target_knowledge_dir = analysis.root_dir.join(".agents").join("knowledge");
    println!("{}", "✨ УСПЕШНО!".green().bold());
    println!("   База знаний инициализирована в: {}", target_knowledge_dir.display().to_string().cyan());
    println!("   • L0 Индекс:       {}", target_knowledge_dir.join("L0_index.json").display());
    println!("   • Карточки L1:     {} файла(ов) в subsystems/", subsystems.len().to_string().yellow());
    println!("   • История ADR:     {}", target_knowledge_dir.join("adr/0001_initial_architecture.md").display());

    println!("\n💡 Что делать дальше:");
    println!("   1. Откройте {} и скорректируйте правила core_rules при необходимости.", "L0_index.json".cyan());
    println!("   2. Теперь любой AI-агент в этом проекте может читать компактный L0 контекст!");

    Ok(())
}
