use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::types::ProjectAnalysis;

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

#[derive(Deserialize, Debug)]
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
    let modules_str = analysis.detected_modules.join(", ");
    let system_prompt = "Ты опытный технический архитектор. Твоя задача — составить лаконичную карту подсистем для проекта на русском языке. Отвечай СТРОГО в формате JSON array без лишнего текста.";

    let prompt = format!(
        "Проект: {}\nТип: {:?}\nОписание: {}\nОбнаруженные директории и модули: [{}]\n\nСгенерируй JSON массив подсистем с полями: [\"id\", \"name\", \"summary\", \"tags\"].\nSummary должно быть емким (одно предложение), описывающим роль подсистемы.",
        analysis.name, analysis.project_type, analysis.description, modules_str
    );

    let req = OllamaGenerateRequest {
        model,
        prompt: &prompt,
        system: system_prompt,
        stream: false,
        format: "json",
    };

    let res = ureq::post(&format!("{}/api/generate", ollama_url))
        .timeout(Duration::from_secs(60))
        .send_json(&req)
        .with_context(|| "Ошибка сетевого запроса к Ollama")?;

    let body: OllamaGenerateResponse = res
        .into_json()
        .with_context(|| "Ошибка декодирования ответа Ollama")?;

    let parsed: Vec<SubsystemAiSummary> = serde_json::from_str(&body.response)
        .with_context(|| format!("Не удалось распарсить JSON подсистем от Ollama: {}", body.response))?;

    Ok(parsed)
}
