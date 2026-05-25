use crate::config::PromptTemplateConfig;
use anyhow::{bail, Result};
use rand::{seq::SliceRandom, Rng};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct GeneratedPrompt {
    pub template_id: String,
    pub prompt_hash: String,
    pub text: String,
}

pub fn generate(prompts: &[PromptTemplateConfig]) -> Result<GeneratedPrompt> {
    let mut rng = rand::thread_rng();
    let Some(template) = prompts.choose(&mut rng) else {
        bail!("at least one prompt template is required");
    };

    let text = template
        .text
        .replace("{{topic}}", choose(&mut rng, TOPICS))
        .replace("{{word}}", choose(&mut rng, WORDS))
        .replace("{{style}}", choose(&mut rng, STYLES))
        .replace("{{number}}", &rng.gen_range(1..99).to_string());

    let prompt_hash = hash_prompt(&text);
    Ok(GeneratedPrompt {
        template_id: template.id.clone(),
        prompt_hash,
        text,
    })
}

fn choose<'a>(rng: &mut impl Rng, values: &'a [&str]) -> &'a str {
    values
        .choose(rng)
        .expect("static prompt values are non-empty")
}

fn hash_prompt(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    let digest = hasher.finalize();
    format!("{digest:x}")[..16].to_string()
}

const TOPICS: &[&str] = &["rain", "maps", "tea", "clocks", "notebooks", "bridges"];
const WORDS: &[&str] = &["small", "steady", "bright", "simple", "clear", "quiet"];
const STYLES: &[&str] = &["plain", "formal", "friendly", "technical", "neutral"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_prompt_has_metadata_and_no_placeholders() {
        let prompts = vec![PromptTemplateConfig {
            id: "test".to_string(),
            text: "Say {{word}} about {{topic}} after {{number}} in {{style}}.".to_string(),
        }];
        let prompt = generate(&prompts).unwrap();
        assert_eq!(prompt.template_id, "test");
        assert_eq!(prompt.prompt_hash.len(), 16);
        assert!(!prompt.text.contains("{{"));
    }
}
