use sentry::protocol::{Event, Stacktrace, Value};

pub fn init(dsn: Option<sentry::types::Dsn>, environment: &str) -> sentry::ClientInitGuard {
    let replacements = private_path_values();
    let mut options = sentry::ClientOptions::new()
        .release(format!("slate-modpack-api@{}", env!("CARGO_PKG_VERSION")))
        .environment(environment.to_owned())
        .send_default_pii(false)
        .attach_stacktrace(true)
        .max_breadcrumbs(50)
        .before_send(move |event| Some(sanitize_event(event, &replacements)));
    options.dsn = dsn;
    sentry::init(options)
}

fn private_path_values() -> Vec<String> {
    let mut values = Vec::new();
    for variable in ["USERPROFILE", "HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            values.push(value.to_string_lossy().into_owned());
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn sanitize_event(event: Event<'static>, replacements: &[String]) -> Event<'static> {
    let mut value = match serde_json::to_value(&event) {
        Ok(value) => value,
        Err(_) => return minimal_event(event),
    };
    sanitize_json_strings(&mut value, replacements);
    let mut event = serde_json::from_value(value).unwrap_or_else(|_| minimal_event(event));
    event.user = None;
    event.request = None;
    event.server_name = None;
    event.extra.clear();
    event.tags.clear();
    event
        .contexts
        .retain(|key, _| matches!(key.as_str(), "device" | "os" | "runtime"));
    for breadcrumb in &mut event.breadcrumbs.values {
        breadcrumb.data.clear();
    }
    sanitize_stacktrace(event.stacktrace.as_mut());
    for exception in &mut event.exception.values {
        sanitize_stacktrace(exception.stacktrace.as_mut());
        sanitize_stacktrace(exception.raw_stacktrace.as_mut());
    }
    for thread in &mut event.threads.values {
        thread.name = None;
        sanitize_stacktrace(thread.stacktrace.as_mut());
        sanitize_stacktrace(thread.raw_stacktrace.as_mut());
    }
    event
}

fn minimal_event(event: Event<'static>) -> Event<'static> {
    let mut sanitized = Event::new();
    sanitized.event_id = event.event_id;
    sanitized.level = event.level;
    sanitized.release = event.release;
    sanitized.environment = event.environment;
    sanitized.platform = event.platform;
    sanitized.message = Some("slate captured an error that could not be sanitized".to_owned());
    sanitized
}

fn sanitize_stacktrace(stacktrace: Option<&mut Stacktrace>) {
    let Some(stacktrace) = stacktrace else {
        return;
    };
    stacktrace.registers.clear();
    for frame in &mut stacktrace.frames {
        frame.abs_path = None;
        frame.vars.clear();
    }
}

fn sanitize_json_strings(value: &mut Value, replacements: &[String]) {
    match value {
        Value::String(text) => *text = sanitize_text(text, replacements),
        Value::Array(values) => {
            for value in values {
                sanitize_json_strings(value, replacements);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                sanitize_json_strings(value, replacements);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn sanitize_text(text: &str, replacements: &[String]) -> String {
    let mut sanitized = text.to_owned();
    for replacement in replacements {
        sanitized = sanitized.replace(replacement, "<private-path>");
        sanitized = sanitized.replace(&replacement.replace('\\', "/"), "<private-path>");
    }
    for marker in [
        "access_token=",
        "refresh_token=",
        "client_secret=",
        "authorization=",
        "Bearer ",
        "\"access_token\":\"",
        "\"refresh_token\":\"",
        "\"client_secret\":\"",
        "\"authorization\":\"",
    ] {
        sanitized = redact_marker_value(&sanitized, marker);
    }
    sanitized
}

fn redact_marker_value(text: &str, marker: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(index) = remaining.find(marker) {
        let value_start = index + marker.len();
        output.push_str(&remaining[..value_start]);
        output.push_str("[redacted]");
        let tail = &remaining[value_start..];
        let value_end = tail
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '&' | '"' | '\'')
            })
            .unwrap_or(tail.len());
        remaining = &tail[value_end..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::sanitize_event;
    use sentry::protocol::{Event, Request};

    #[test]
    fn server_events_remove_request_and_private_paths() {
        let mut event = Event::new();
        event.message = Some("failed under /home/private/slate Bearer visible".to_owned());
        event.request = Some(Request::default());

        let event = sanitize_event(event, &["/home/private".to_owned()]);

        assert!(event.request.is_none());
        assert!(event.user.is_none());
        assert!(event.message.is_some_and(|message| {
            !message.contains("/home/private") && !message.contains("visible")
        }));
    }
}
