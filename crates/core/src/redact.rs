use serde_json::Value;

const MASK: &str = "***";
const SECRET_NAME_PARTS: [&str; 5] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD"];
const TOKEN_PREFIXES: [&str; 14] = [
    "sk-",
    "sk_live_",
    "rk_live_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "npm_",
    "xox",
    "AKIA",
    "AIza",
];
const MIN_PREFIXED_TOKEN_LEN: usize = 20;
const JWT_START: &str = "eyJ";
const PEM_START: &str = "-----BEGIN ";
const PEM_DASHES: &str = "-----";

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

pub fn redact_json(name: &str, value: &Value) -> Value {
    if is_secret_name(name) {
        return Value::from(MASK);
    }
    match value {
        Value::String(text) => Value::String(redact(text)),
        Value::Array(items) => items.iter().map(|item| redact_json(name, item)).collect(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, item)| (key.clone(), redact_json(key, item)))
            .collect(),
        _ => value.clone(),
    }
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
    .or_else(|| private_key_block(text))
    .or_else(|| url_password(text))
    .or_else(|| prefixed_token(text))
    .or_else(|| json_web_token(text))
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
    let kept = label.len() + prefix_len(&text[label.len()..], is_blank);
    let value_len = prefix_len(&text[kept..], |character| !ends_value(character));
    (value_len > 0).then_some((kept, kept + value_len))
}

fn prefixed_token(text: &str) -> Option<(usize, usize)> {
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| text.starts_with(*prefix))?;
    let token_len = prefix_len(text, is_token_char);
    (token_len >= MIN_PREFIXED_TOKEN_LEN).then_some((0, token_len))
}

fn json_web_token(text: &str) -> Option<(usize, usize)> {
    if !text.starts_with(JWT_START) {
        return None;
    }
    let token_len = prefix_len(text, |character| {
        is_token_char(character) || character == '.'
    });
    (token_len >= MIN_PREFIXED_TOKEN_LEN && text[..token_len].matches('.').count() >= 2)
        .then_some((0, token_len))
}

fn private_key_block(text: &str) -> Option<(usize, usize)> {
    if !text.starts_with(PEM_START) {
        return None;
    }
    let header_end = PEM_START.len() + text[PEM_START.len()..].find(PEM_DASHES)?;
    if !text[..header_end].ends_with("PRIVATE KEY") {
        return None;
    }
    let kept = header_end + PEM_DASHES.len();
    let end = text[kept..]
        .find("-----END ")
        .and_then(|footer| {
            let after_dashes = kept + footer + PEM_DASHES.len();
            text[after_dashes..]
                .find(PEM_DASHES)
                .map(|closing| after_dashes + closing + PEM_DASHES.len())
        })
        .unwrap_or(text.len());
    (end > kept).then_some((kept, end))
}

fn url_password(text: &str) -> Option<(usize, usize)> {
    let scheme_len = prefix_len(text, |character| {
        character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '-')
    });
    if scheme_len == 0 || !text[scheme_len..].starts_with("://") {
        return None;
    }
    let user_start = scheme_len + 3;
    let user_end = user_start
        + prefix_len(&text[user_start..], |character| {
            !character.is_whitespace()
                && !matches!(character, '/' | '@' | ':' | '?' | '#' | '"' | '\'')
        });
    if !text[user_end..].starts_with(':') {
        return None;
    }
    let kept = user_end + 1;
    let end = kept
        + prefix_len(&text[kept..], |character| {
            !character.is_whitespace() && !matches!(character, '/' | '@' | '?' | '#' | '"' | '\'')
        });
    (end > kept && text[end..].starts_with('@')).then_some((kept, end))
}

fn secret_assignment(text: &str) -> Option<(usize, usize)> {
    let name_len = prefix_len(text, is_name_char);
    if !is_secret_name(&text[..name_len]) {
        return None;
    }
    let quoted_name = text[name_len..].starts_with(is_quote);
    let after_name = name_len + usize::from(quoted_name);
    let operator = after_name + prefix_len(&text[after_name..], is_blank);
    let operator_char = text[operator..].chars().next();
    let value_start = operator + 1 + prefix_len(text.get(operator + 1..).unwrap_or(""), is_blank);
    let kept = match operator_char {
        Some('=') => value_start,
        Some(':') if quoted_name || text[value_start..].starts_with(is_quote) => value_start,
        _ if text.starts_with('-') && operator > name_len && operator_char != Some('-') => operator,
        _ => return None,
    };
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

fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_PARTS.iter().any(|part| upper.contains(part))
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

fn is_blank(character: char) -> bool {
    matches!(character, ' ' | '\t')
}

fn is_token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}

#[cfg(test)]
mod tests {
    use super::{redact, redact_json};
    use serde_json::json;

    #[test]
    fn a_value_tree_is_masked_string_by_string_before_it_is_turned_into_json() {
        let input = json!({
            "config": "{\"api_key\": \"abc123secret\"}",
            "list": ["Bearer abc", {"client_secret": {"a": 1}}],
            "note": "password: \"hunter2\"",
            "token": 5,
            "count": 3,
        });
        assert_eq!(
            redact_json("", &input).to_string(),
            r#"{"config":"{\"api_key\": ***}","list":["Bearer ***",{"client_secret":"***"}],"note":"password: ***","token":"***","count":3}"#
        );
        assert_eq!(
            redact_json("", &json!("GITHUB_TOKEN=abc123 npm publish")),
            json!("GITHUB_TOKEN=*** npm publish")
        );
        assert_eq!(redact_json("api_key", &json!("abc")), json!("***"));
        assert_eq!(redact_json("", &json!(null)), json!(null));
    }

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
    fn spaced_quoted_and_flag_forms_are_masked() {
        assert_eq!(
            redact(r#"$env:GITHUB_TOKEN = "abc"; git push"#),
            "$env:GITHUB_TOKEN = ***; git push"
        );
        assert_eq!(
            redact(r#"{"api_key":"abc123","n":1}"#),
            r#"{"api_key":***,"n":1}"#
        );
        assert_eq!(redact(r#"password: "hunter2""#), "password: ***");
        assert_eq!(
            redact("mysql --password hunter2 db"),
            "mysql --password *** db"
        );
        for text in [
            "apiKey: string",
            r#"git commit -m "fix: token refresh""#,
            "--api-key",
        ] {
            assert_eq!(redact(text), text);
        }
    }

    #[test]
    fn url_credentials_lose_their_password() {
        assert_eq!(
            redact("psql postgres://user:pw@host/db"),
            "psql postgres://user:***@host/db"
        );
        assert_eq!(
            redact("curl https://host:8080/x"),
            "curl https://host:8080/x"
        );
    }

    #[test]
    fn more_token_shapes_are_masked() {
        assert_eq!(
            redact("github_pat_11ABCDEFG0123456789_abcdefghij x"),
            "*** x"
        );
        assert_eq!(redact("AKIAIOSFODNN7EXAMPLE"), "***");
        assert_eq!(redact("glpat-abcdefghij0123456789"), "***");
        assert_eq!(
            redact("token eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.sig_value"),
            "token ***"
        );
        assert_eq!(
            redact("a\n-----BEGIN PRIVATE KEY-----\nMIIabc\n-----END PRIVATE KEY-----\nb"),
            "a\n-----BEGIN PRIVATE KEY-----***\nb"
        );
    }

    #[test]
    fn non_ascii_text_survives() {
        assert_eq!(redact("é API_KEY=ж日本 ü"), "é API_KEY=*** ü");
    }
}
