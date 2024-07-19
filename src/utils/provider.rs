use reqwest::get;
use std::env;

/// Checks for OpenAI API key.
pub fn check_openai() -> Result<(), String> {
    const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

    if env::var(OPENAI_API_KEY).is_err() {
        return Err("OpenAI API key not found".into());
    }

    Ok(())
}

/// Checks for Ollama running at the default port.
pub async fn check_ollama(host: &str, port: u16) -> Result<(), String> {
    let ollama_url = format!("{}:{}", host, port);

    let response = get(&ollama_url).await.map_err(|e| format!("{}", e))?;

    if let Ok(text) = response.text().await {
        // Ollama returns this text specifically
        if text == "Ollama is running" {
            return Ok(());
        }
    }
    Err(format!(
        "Something is running at {} but its not Ollama?",
        ollama_url
    ))
}
