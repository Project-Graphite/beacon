const MASK: &str = "***";
const SECRET_NAME_PARTS: [&str; 5] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD"];
const TOKEN_PREFIXES: [&str; 3] = ["sk-", "ghp_", "xox"];
const MIN_PREFIXED_TOKEN_LEN: usize = 20;

pub fn redact(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    let mut at_word_start = true;
    while let Some(character) = rest.chars().next() {
        if at_word_start && let Some((kept, consumed)) = secret_at(rest) {
            redacted.push_str(&rest[..kept]);
            redacted.push_str(MASK);
            rest = &rest[consumed..];
            at_word_start = false;
            continue;
        }
        redacted.push(character);
        rest = &rest[character.len_utf8()..];
        at_word_start = !is_name_char(character);
    }
    redacted
}

fn secret_at(text: &str) -> Option<(usize, usize)> {
    header_value(text, "authorization:", |character| {
        matches!(character, '"' | '\'' | '\r' | '\n')
    })
    .or_else(|| {
        header_value(text, "bearer", |character| {
            character.is_whitespace() || is_quote(character)
        })
        .filter(|&(kept, _)| kept > "bearer".len())
    })
    .or_else(|| prefixed_token(text))
    .or_else(|| secret_assignment(text))
}

fn header_value(
    text: &str,
    label: &str,
    ends_value: impl Fn(char) -> bool,
) -> Option<(usize, usize)> {
    let head = text.get(..label.len())?;
    if !head.eq_ignore_ascii_case(label) {
        return None;
    }
    let kept = label.len()
        + prefix_len(&text[label.len()..], |character| {
            matches!(character, ' ' | '\t')
        });
    let value_len = prefix_len(&text[kept..], |character| !ends_value(character));
    (value_len > 0).then_some((kept, kept + value_len))
}

fn prefixed_token(text: &str) -> Option<(usize, usize)> {
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| text.starts_with(*prefix))?;
    let token_len = prefix_len(text, |character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
    });
    (token_len >= MIN_PREFIXED_TOKEN_LEN).then_some((0, token_len))
}

fn secret_assignment(text: &str) -> Option<(usize, usize)> {
    let name_len = prefix_len(text, is_name_char);
    let name = text[..name_len].to_ascii_uppercase();
    if !text[name_len..].starts_with('=')
        || !SECRET_NAME_PARTS.iter().any(|part| name.contains(part))
    {
        return None;
    }
    let kept = name_len + 1;
    let value = &text[kept..];
    let value_len = match value.chars().next() {
        Some(quote) if is_quote(quote) => value[1..]
            .find(quote)
            .map_or(value.len(), |closing| closing + 2),
        _ => prefix_len(value, |character| {
            !character.is_whitespace() && !is_quote(character)
        }),
    };
    (value_len > 0).then_some((kept, kept + value_len))
}

fn prefix_len(text: &str, keep: impl Fn(char) -> bool) -> usize {
    text.find(|character| !keep(character))
        .unwrap_or(text.len())
}

fn is_name_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
}

fn is_quote(character: char) -> bool {
    matches!(character, '"' | '\'')
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn secret_assignments_keep_the_name_and_mask_the_value() {
        assert_eq!(
            redact("GITHUB_TOKEN=abc123 npm publish"),
            "GITHUB_TOKEN=*** npm publish"
        );
        assert_eq!(
            redact(r#"$env:OPENAI_API_KEY="s3cr3t value"; run"#),
            "$env:OPENAI_API_KEY=***; run"
        );
        assert_eq!(
            redact("mysql --password='p w' db"),
            "mysql --password=*** db"
        );
        assert_eq!(redact("DB_PASSWD=hunter2"), "DB_PASSWD=***");
        assert_eq!(redact("client_secret=xyz&a=1"), "client_secret=***");
        assert_eq!(redact("set private_key=-----BEGIN"), "set private_key=***");
    }

    #[test]
    fn ordinary_assignments_are_left_alone() {
        for text in [
            "NODE_ENV=production npm start",
            "count=3",
            "KEY=",
            "echo TOKEN is unset",
            "cd C:\\keys && dir",
        ] {
            assert_eq!(redact(text), text);
        }
    }

    #[test]
    fn authorization_headers_and_bearer_tokens_are_masked() {
        assert_eq!(
            redact(r#"curl -H "Authorization: Basic dXNlcjpwYXNz" https://x.test"#),
            r#"curl -H "Authorization: ***" https://x.test"#
        );
        assert_eq!(
            redact("authorization: token abc\nnext line"),
            "authorization: ***\nnext line"
        );
        assert_eq!(
            redact("curl -H 'Bearer eyJhbGciOi.payload.sig' x"),
            "curl -H 'Bearer ***' x"
        );
        assert_eq!(redact("Bearer"), "Bearer");
    }

    #[test]
    fn long_prefixed_tokens_are_masked() {
        assert_eq!(
            redact("use sk-ant-api03-abcdefghijklmnop now"),
            "use *** now"
        );
        assert_eq!(
            redact("git push https://ghp_0123456789abcdefABCDEF@github.com/x"),
            "git push https://***@github.com/x"
        );
        assert_eq!(redact("xoxb-123456789012-abcdefghij"), "***");
        assert_eq!(
            redact("sk-short and task-sk-0123456789abcdefghij"),
            "sk-short and task-sk-0123456789abcdefghij"
        );
    }

    #[test]
    fn non_ascii_text_survives() {
        assert_eq!(redact("é API_KEY=ж日本 ü"), "é API_KEY=*** ü");
    }
}
