pub static SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");
pub static USER_PROMPT_TEMPLATE: &str = include_str!("user_prompt.txt");

pub fn system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

/// Generate the user prompt for a diff.
///
/// The model reads the full diff directly; cmt no longer pre-digests it with a
/// hand-rolled analysis layer (see the dropped `analysis` module).
pub fn user_prompt(changes: &str) -> String {
    USER_PROMPT_TEMPLATE.replace("{{changes}}", changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_prompt_embeds_diff() {
        let prompt = user_prompt("test diff");
        assert!(prompt.contains("test diff"));
        assert!(!prompt.contains("Pre-Analysis"));
    }
}
