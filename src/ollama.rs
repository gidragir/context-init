use crate::types::ProjectAnalysis;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
    format: &'a str,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SubsystemAiSummary {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
}

pub fn is_ollama_online(url: &str) -> bool {
    ureq::get(&format!("{}/api/tags", url))
        .timeout(Duration::from_millis(1500))
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

pub fn enrich_subsystems_with_ai(
    analysis: &ProjectAnalysis,
    ollama_url: &str,
    model: &str,
) -> Result<Vec<SubsystemAiSummary>> {
    // Подготавливаем заземленные факты по каждой подсистеме
    let mut modules_overview = String::new();
    for m in &analysis.detected_modules {
        let files_list: Vec<String> = m
            .files
            .iter()
            .take(5)
            .map(|f| format!("{} ({} lines)", f.path, f.lines))
            .collect();
        let symbols_list: Vec<String> = m.symbols.iter().take(5).cloned().collect();

        modules_overview.push_str(&format!(
            "\n- Подсистема `{}` (директория: `{}`):\n  Ключевые файлы: [{}]\n  Точки входа: [{}]\n  Сигнатуры/экспорты: [{}]\n",
            m.name,
            m.dir_path,
            files_list.join(", "),
            m.entrypoints.join(", "),
            symbols_list.join(", ")
        ));
    }

    let system_prompt = "# LANGUAGE MANDATE: STRICTLY RUSSIAN\nТы ведущий системный архитектор. Твоя задача — составить фактологическую и сжатую карту подсистем проекта для ИИ-агентов. Опирайся строго на переданные файлы и сигнатуры кода. Запрещены абстрактные фразы. Отвечай СТРОГО валидным JSON-объектом вида: {\"subsystems\": [{\"id\": \"...\", \"name\": \"...\", \"summary\": \"...\", \"tags\": [\"...\"]}]}.";

    let prompt = format!(
        "Проект: {}\nСтек: {:?}\nОписание: {}\n\nФактическая структура модулей:{}\n\nСгенерируй JSON-объект: {{\"subsystems\": [...]}}.\nДля поля id используй нормализованный идентификатор (например: core, detector, api).\nВ summary четко сформулируй архитектурное назначение (1 емкое предложение).\nВ tags укажи 2-4 технических тега.",
        analysis.name, analysis.project_type, analysis.description, modules_overview
    );

    let req = OllamaGenerateRequest {
        model,
        prompt: &prompt,
        system: system_prompt,
        stream: false,
        format: "json",
    };

    let res = ureq::post(&format!("{}/api/generate", ollama_url))
        .timeout(Duration::from_secs(90))
        .send_json(&req)
        .with_context(|| "Ошибка сетевого запроса к Ollama")?;

    let body: OllamaGenerateResponse = res
        .into_json()
        .with_context(|| "Ошибка декодирования ответа Ollama")?;

    let raw = body.response.trim();
    let json_clean = if raw.starts_with("```") {
        raw.trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        raw
    };

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(alias = "items", alias = "modules")]
        subsystems: Vec<SubsystemAiSummary>,
    }

    let parsed: Vec<SubsystemAiSummary> = if let Ok(w) = serde_json::from_str::<Wrapper>(json_clean)
    {
        w.subsystems
    } else if let Ok(arr) = serde_json::from_str::<Vec<SubsystemAiSummary>>(json_clean) {
        arr
    } else if let Ok(single) = serde_json::from_str::<SubsystemAiSummary>(json_clean) {
        vec![single]
    } else {
        serde_json::from_str(json_clean).with_context(|| {
            format!(
                "Не удалось распарсить JSON подсистем от Ollama: {}",
                body.response
            )
        })?
    };

    Ok(parsed)
}
