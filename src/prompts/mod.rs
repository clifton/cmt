pub static SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");
pub static USER_PROMPT_TEMPLATE: &str = include_str!("user_prompt.txt");

pub fn system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

/// Generate user prompt with optional analysis context
///
/// When analysis is provided, it's included before the diff to give the AI
/// structured context about the changes. This helps with better type classification.
pub fn user_prompt(changes: &str, analysis: Option<&str>) -> String {
    let mut prompt = String::new();

    // Include analysis if provided
    if let Some(analysis_text) = analysis {
        prompt.push_str("# Pre-Analysis of Changes\n\n");
        prompt.push_str("The following analysis was generated automatically from the diff.\n");
        prompt.push_str("Use this to inform your commit type selection, but always verify by reading the actual diff.\n\n");
        prompt.push_str(analysis_text);
        prompt.push_str("\n---\n\n");
    }

    prompt.push_str(&USER_PROMPT_TEMPLATE.replace("{{changes}}", changes));
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_prompt_without_analysis() {
        let prompt = user_prompt("test diff", None);
        assert!(prompt.contains("test diff"));
        assert!(!prompt.contains("Pre-Analysis"));
    }

    #[test]
    fn test_user_prompt_with_analysis() {
        let prompt = user_prompt("test diff", Some("## Change Summary\n1 file changed"));
        assert!(prompt.contains("test diff"));
        assert!(prompt.contains("Pre-Analysis"));
        assert!(prompt.contains("Change Summary"));
    }
}
