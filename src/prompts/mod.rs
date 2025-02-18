pub static SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");
pub static USER_PROMPT_TEMPLATE: &str = include_str!("user_prompt.txt");

pub fn system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

pub fn user_prompt(changes: &str) -> String {
    USER_PROMPT_TEMPLATE.replace("{{changes}}", changes)
}
