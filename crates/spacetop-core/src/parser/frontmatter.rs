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
