use super::{SummaryError, SummaryProvider};
use serde_json::Value;

pub fn parse(provider: SummaryProvider, bytes: &[u8]) -> Result<String, SummaryError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SummaryError::Output)?;
    let summary = match provider {
        SummaryProvider::Claude => {
            let value: Value = serde_json::from_str(text).map_err(|_| SummaryError::Output)?;
            if value.get("is_error").and_then(Value::as_bool) != Some(false) {
                return Err(SummaryError::Provider);
            }
            value
                .get("result")
                .and_then(Value::as_str)
                .ok_or(SummaryError::Output)?
                .to_owned()
        }
        SummaryProvider::Codex => {
            let mut final_message = None;
            let mut completed = false;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let event: Value = serde_json::from_str(line).map_err(|_| SummaryError::Output)?;
                match event.get("type").and_then(Value::as_str) {
                    Some("error" | "turn.failed") => return Err(SummaryError::Provider),
                    Some("turn.completed") => completed = true,
                    Some("item.completed") if event["item"]["type"] == "agent_message" => {
                        final_message = event["item"]["text"].as_str().map(str::to_owned);
                    }
                    _ => {}
                }
            }
            if !completed {
                return Err(SummaryError::Output);
            }
            final_message.ok_or(SummaryError::Output)?
        }
    };
    let safe = sanitize(&summary);
    if safe.trim().is_empty() {
        return Err(SummaryError::Output);
    }
    Ok(safe)
}

/// Plain text only: drop control sequences, including OSC hyperlinks/clipboard,
/// CSI, DCS, and C1 equivalents. Never hand provider text to an ANSI renderer.
pub fn sanitize(text: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        Escape,
        Csi,
        String,
        StringEscape,
    }
    let mut state = State::Text;
    let mut safe = String::new();
    for ch in text.chars() {
        state = match (state, ch) {
            (State::Text, '\x1b') => State::Escape,
            (State::Text, '\u{009b}') => State::Csi,
            (State::Text, '\u{0090}' | '\u{009d}' | '\u{009e}' | '\u{009f}') => State::String,
            (State::Text, '\n' | '\t') => {
                safe.push(ch);
                State::Text
            }
            (State::Text, c) => {
                if !c.is_control() {
                    safe.push(c);
                }
                State::Text
            }
            (State::Escape, '[') => State::Csi,
            (State::Escape, ']' | 'P' | '^' | '_') => State::String,
            (State::Escape, _) => State::Text,
            (State::Csi, c) if ('@'..='~').contains(&c) => State::Text,
            (State::Csi, _) => State::Csi,
            (State::String, '\x07' | '\u{009c}') => State::Text,
            (State::String, '\x1b') => State::StringEscape,
            (State::String, _) => State::String,
            (State::StringEscape, '\\') => State::Text,
            (State::StringEscape, _) => State::String,
        };
    }
    safe.trim().to_owned()
}
