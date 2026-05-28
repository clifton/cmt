//! Redact likely secrets from the diff before it is sent to a third-party LLM.
//!
//! `.cmtignore` is path-based, so a credential hardcoded *inside* an otherwise
//! legitimate staged file (an accidentally-staged `.env`, an AWS key, a GitHub
//! token, a private key block) would still be sent to the model. This pass
//! scrubs high-precision secret patterns from the diff text, replacing them
//! with `[REDACTED]`. It is deliberately conservative (named, structured tokens
//! plus `key = value` assignments) to keep false positives low, and can be
//! disabled with `--no-redact`.

use regex::Regex;
use std::sync::OnceLock;

const PLACEHOLDER: &str = "[REDACTED]";

/// Patterns whose entire match is a secret and is replaced wholesale.
fn full_match_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // AWS access key id
            r"AKIA[0-9A-Z]{16}",
            // GitHub tokens (ghp_, gho_, ghu_, ghs_, ghr_)
            r"\bgh[pousr]_[A-Za-z0-9]{36,}\b",
            // GitLab personal access token
            r"\bglpat-[A-Za-z0-9_-]{20,}\b",
            // Slack tokens
            r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b",
            // OpenAI-style keys
            r"\bsk-[A-Za-z0-9_-]{20,}\b",
            // JWT (header.payload.signature)
            r"\beyJ[A-Za-z0-9_-]{6,}\.eyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\b",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("valid secret regex"))
        .collect()
    })
}

/// `key = "value"` style assignment: capture 1 keeps the key + separator (+ any
/// opening quote), capture 2 is the secret value that gets replaced.
fn assignment_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)((?:api[_-]?key|secret|client[_-]?secret|access[_-]?key|auth[_-]?token|token|password|passwd)["']?\s*[:=]\s*["']?)([A-Za-z0-9+/_\-]{12,})"#,
        )
        .expect("valid assignment regex")
    })
}

/// Redact likely secrets from `input`. Returns the scrubbed text and the number
/// of redactions made.
pub fn redact_secrets(input: &str) -> (String, usize) {
    let mut count = 0usize;
    let mut out = String::with_capacity(input.len());
    let mut in_pem = false;

    for line in input.split_inclusive('\n') {
        let (line_out, n, next_pem) = redact_line(line, in_pem);
        count += n;
        in_pem = next_pem;
        out.push_str(&line_out);
    }

    (out, count)
}

/// Redact a single line, tracking whether we are inside a multi-line PEM private
/// key block (whose base64 body lines aren't caught by the token patterns).
fn redact_line(line: &str, in_pem: bool) -> (String, usize, bool) {
    let is_private_key_marker = line.contains("PRIVATE KEY-----");

    if in_pem {
        if is_private_key_marker && line.contains("END") {
            // Keep the END marker; leave the block.
            return (line.to_string(), 0, false);
        }
        // Redact the base64 body line, preserving a diff prefix and newline.
        return (redact_body_line(line), 1, true);
    }

    if is_private_key_marker && line.contains("BEGIN") {
        // Keep the BEGIN marker (not itself a secret) and enter the block.
        return (line.to_string(), 0, true);
    }

    let mut s = line.to_string();
    let mut n = 0usize;

    for re in full_match_patterns() {
        let matches = re.find_iter(&s).count();
        if matches > 0 {
            n += matches;
            s = re.replace_all(&s, PLACEHOLDER).into_owned();
        }
    }

    let re = assignment_pattern();
    let matches = re.find_iter(&s).count();
    if matches > 0 {
        n += matches;
        s = re
            .replace_all(&s, |caps: &regex::Captures| {
                format!("{}{}", &caps[1], PLACEHOLDER)
            })
            .into_owned();
    }

    (s, n, false)
}

/// Replace a line's content with the placeholder while preserving a leading diff
/// marker (`+`/`-`/space) and any trailing newline.
fn redact_body_line(line: &str) -> String {
    let (newline, body) = match line.strip_suffix('\n') {
        Some(rest) => ("\n", rest),
        None => ("", line),
    };
    let prefix = match body.chars().next() {
        Some(c @ ('+' | '-' | ' ')) => &body[..c.len_utf8()],
        _ => "",
    };
    format!("{prefix}{PLACEHOLDER}{newline}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacts_named_tokens() {
        let cases = [
            "+const key = AKIA1234567890ABCDEF;",
            "+ghp_0123456789012345678901234567890123456789",
            "+token: glpat-abcdefabcdefabcdef1234",
            "+let s = \"sk-abcdefghijklmnopqrstuvwxyz0123\";",
        ];
        for c in cases {
            let (out, n) = redact_secrets(c);
            assert!(n >= 1, "expected redaction in: {c}");
            assert!(out.contains(PLACEHOLDER), "no placeholder in: {out}");
        }
    }

    #[test]
    fn test_redacts_assignment_value_keeps_key() {
        let (out, n) = redact_secrets("+API_KEY = \"s3cr3tvalue1234567890\"");
        assert_eq!(n, 1);
        assert!(out.contains("API_KEY"), "key name should survive: {out}");
        assert!(out.contains(PLACEHOLDER), "value should be redacted: {out}");
        assert!(
            !out.contains("s3cr3tvalue1234567890"),
            "secret leaked: {out}"
        );
    }

    #[test]
    fn test_redacts_jwt() {
        let jwt =
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3OCJ9.SflKxwRJSMeKKF2QT4fwpM";
        let (out, n) = redact_secrets(&format!("+const auth = {jwt};"));
        assert!(n >= 1, "JWT should be redacted: {out}");
        assert!(out.contains(PLACEHOLDER));
        assert!(!out.contains("eyJzdWIi"), "JWT payload leaked: {out}");
    }

    #[test]
    fn test_redacts_pem_block_body() {
        let diff = "\
+-----BEGIN RSA PRIVATE KEY-----
+MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDexample
+abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ012345
+-----END RSA PRIVATE KEY-----
";
        let (out, n) = redact_secrets(diff);
        assert!(n >= 2, "both body lines should be redacted: {out}");
        // Markers are kept so the reader knows a key was present.
        assert!(out.contains("BEGIN RSA PRIVATE KEY"));
        assert!(out.contains("END RSA PRIVATE KEY"));
        // Key material is gone.
        assert!(!out.contains("MIIEvQIBAD"), "key body leaked: {out}");
        // Diff prefix preserved on redacted body lines.
        assert!(out.lines().any(|l| l == format!("+{PLACEHOLDER}")));
    }

    #[test]
    fn test_leaves_ordinary_diff_untouched() {
        let diff = "\
+fn add(a: i32, b: i32) -> i32 {
+    a + b
+}
-let x = 42;
";
        let (out, n) = redact_secrets(diff);
        assert_eq!(n, 0, "no false positives expected");
        assert_eq!(out, diff);
    }

    #[test]
    fn test_short_values_not_redacted() {
        // A short token=... assignment shouldn't trip the 12+ char value rule.
        let (out, n) = redact_secrets("+token = \"abc\"");
        assert_eq!(n, 0, "short value should not be redacted: {out}");
    }
}
