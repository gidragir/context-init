use crate::types::{L0Index, ProjectAnalysis, SubsystemInfo};
use anyhow::{Context, Result};
use std::fs;

pub fn generate_knowledge_base(
    analysis: &ProjectAnalysis,
    subsystems: Vec<SubsystemInfo>,
    force: bool,
) -> Result<()> {
    let base_dir = analysis.root_dir.join(".agents").join("knowledge");
    let subsystems_dir = base_dir.join("subsystems");
    let cli_dir = base_dir.join("cli");
    let adr_dir = base_dir.join("adr");

    if base_dir.exists() && !force {
        anyhow::bail!(
            "Директория {:?} уже существует. Используйте флаг --force для перезаписи.",
            base_dir
        );
    }

    fs::create_dir_all(&subsystems_dir)
        .with_context(|| format!("Не удалось создать {:?}", subsystems_dir))?;
    fs::create_dir_all(&cli_dir).with_context(|| format!("Не удалось создать {:?}", cli_dir))?;
    fs::create_dir_all(&adr_dir).with_context(|| format!("Не удалось создать {:?}", adr_dir))?;

    // 1. Создаем L0_index.json
    let today = chrono_like_date();
    let l0_index = L0Index {
        schema: "https://json-schema.org/draft/2020-12/schema".to_string(),
        project: analysis.name.clone(),
        description: analysis.description.clone(),
        root_path: analysis.root_dir.to_string_lossy().to_string(),
        updated_at: today.clone(),
        project_type: analysis.project_type.display_name().to_string(),
        subsystems: subsystems.clone(),
        core_rules: analysis.rules.clone(),
    };

    let l0_json = serde_json::to_string_pretty(&l0_index)?;
    let l0_path = base_dir.join("L0_index.json");
    fs::write(&l0_path, l0_json).with_context(|| format!("Не удалось записать {:?}", l0_path))?;

    // 2. Создаем карточки подсистем L1
    for sub in &subsystems {
        let sub_file = base_dir.join(&sub.l1_path);
        let content = format!(
            "# Подсистема: {} (L1)\n\n## Назначение\n{}\n\n## Ключевые файлы и точки входа\n- Директория: `{}`\n\n## Контракты и зависимости\n- Теги: {}\n\n## Как тестировать и проверять\n- Проверьте сборку и запуск тестов для данного модуля.\n",
            sub.name,
            sub.summary,
            sub.id,
            sub.tags.join(", ")
        );
        fs::write(&sub_file, content)
            .with_context(|| format!("Не удалось записать {:?}", sub_file))?;
    }

    // 3. Создаем начальный ADR (L2)
    let adr_path = adr_dir.join("0001_initial_architecture.md");
    let adr_content = format!(
        "# ADR 0001: Начальная архитектура проекта {}\n\n- **Статус:** Принято\n- **Дата:** {}\n\n## Контекст\nИнициализация проекта {} ({}) и фиксация базовых архитектурных границ.\n\n## Решение\nВнедрение трехуровневой контекстной модели (OpenViking L0/L1/L2) для эффективного взаимодействия с ИИ-ассистентами и разработчиками.\n\n## Последствия\n- Вся ментальная карта зафиксирована в L0_index.json.\n- Подсистемы документируются независимо в subsystems/.\n",
        analysis.name, today, analysis.name, analysis.project_type.display_name()
    );
    fs::write(&adr_path, adr_content)
        .with_context(|| format!("Не удалось записать {:?}", adr_path))?;

    // 4. Создаем базовый справочник команд (L2)
    let cli_path = cli_dir.join("commands.md");
    let cli_content = format!(
        "# Справочник команд: {} (L2)\n\n## Основные команды разработки\n```bash\n# Сборка проекта\n# Запуск тестов\n# Линтинг и форматирование\n```\n",
        analysis.name
    );
    fs::write(&cli_path, cli_content)
        .with_context(|| format!("Не удалось записать {:?}", cli_path))?;

    Ok(())
}

fn chrono_like_date() -> String {
    // Получаем текущую дату в формате YYYY-MM-DD без тяжелых зависимостей
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Приблизительный расчет даты (достаточно для метаданных)
    let mut year = 1970;
    let mut remaining_days = days;
    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &d in &days_in_months {
        if remaining_days >= d {
            remaining_days -= d;
            month += 1;
        } else {
            break;
        }
    }
    let day = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}
