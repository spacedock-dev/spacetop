use super::ParseError;

pub(crate) fn extract_frontmatter<'a>(
    contents: &'a str,
    path: &str,
) -> Result<(&'a str, &'a str), ParseError> {
    match split_frontmatter(contents) {
        Some(SplitFrontmatter::Ok { frontmatter, body }) => Ok((frontmatter, body)),
        Some(SplitFrontmatter::Unterminated) => Err(ParseError::UnterminatedFrontmatter {
            path: path.to_string(),
        }),
        None => Err(ParseError::MissingFrontmatter {
            path: path.to_string(),
        }),
    }
}

pub(crate) enum SplitFrontmatter<'a> {
    Ok { frontmatter: &'a str, body: &'a str },
    Unterminated,
}

/// Split a markdown file's text into its YAML frontmatter block (sans `---` fences)
/// and the body. Returns `None` when no opening `---` fence is present on the first line.
pub(crate) fn split_frontmatter(contents: &str) -> Option<SplitFrontmatter<'_>> {
    let rest = contents
        .strip_prefix("---\r\n")
        .or_else(|| contents.strip_prefix("---\n"))?;
    let body_start = contents.len() - rest.len();

    let remaining = &contents[body_start..];
    let Some(relative_end) = remaining.find("\n---") else {
        return Some(SplitFrontmatter::Unterminated);
    };
    let closing_start = body_start + relative_end + 1;
    let after_marker = closing_start + 3;
    let after_marker = if contents[after_marker..].starts_with("\r\n") {
        after_marker + 2
    } else if contents[after_marker..].starts_with('\n') {
        after_marker + 1
    } else {
        after_marker
    };
    let body = contents[after_marker..]
        .strip_prefix("\r\n")
        .or_else(|| contents[after_marker..].strip_prefix('\n'))
        .unwrap_or(&contents[after_marker..]);

    Some(SplitFrontmatter::Ok {
        frontmatter: &contents[body_start..closing_start],
        body,
    })
}

pub(crate) fn top_level_scalar(frontmatter: &str, field: &str) -> Option<String> {
    top_level_scalar_entries(frontmatter)?
        .into_iter()
        .find_map(|(key, value)| (key == field).then_some(value))
}

pub(crate) fn top_level_scalar_entries(frontmatter: &str) -> Option<Vec<(&str, String)>> {
    let mut entries = Vec::new();

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return None;
        }
        let (key, value) = trimmed.split_once(':')?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        let value = value.trim_start();
        if matches!(
            value.chars().next(),
            Some('[' | '{' | '|' | '>' | '&' | '*')
        ) {
            return None;
        }
        entries.push((key, unquote_scalar(value).to_string()));
    }

    Some(entries)
}

fn unquote_scalar(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}
