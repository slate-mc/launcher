use crate::support_report::sanitize_text;
use sentry::protocol::{Event, Stacktrace, User, Value};
use slate_platform::AppPaths;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub(super) struct CrashReporting {
    enabled: Arc<AtomicBool>,
    installation_id: Arc<RwLock<Option<String>>>,
}

impl CrashReporting {
    pub(super) fn initialize(paths: &AppPaths) -> (Self, sentry::ClientInitGuard) {
        let enabled = Arc::new(AtomicBool::new(false));
        let installation_id = Arc::new(RwLock::new(None));
        let replacements = private_path_values(paths);
        let callback_enabled = enabled.clone();
        let callback_installation_id = installation_id.clone();
        let mut options = sentry::ClientOptions::new()
            .release(format!("slate-desktop@{}", env!("CARGO_PKG_VERSION")))
            .environment(if cfg!(debug_assertions) {
                "development"
            } else {
                "production"
            })
            .send_default_pii(false)
            .attach_stacktrace(true)
            .max_breadcrumbs(40)
            .before_send(move |event| {
                if !callback_enabled.load(Ordering::Acquire) {
                    return None;
                }
                let installation_id = callback_installation_id
                    .read()
                    .ok()
                    .and_then(|value| value.clone());
                Some(sanitize_event(event, &replacements, installation_id))
            });
        if let Some(dsn) =
            option_env!("SLATE_DESKTOP_SENTRY_DSN").and_then(|value| value.parse().ok())
        {
            options.dsn = Some(dsn);
        }
        let guard = sentry::init(options);
        (
            Self {
                enabled,
                installation_id,
            },
            guard,
        )
    }

    pub(super) fn configure(&self, enabled: bool, installation_id: uuid::Uuid) {
        if let Ok(mut current) = self.installation_id.write() {
            *current = Some(installation_id.to_string());
        }
        self.set_enabled(enabled);
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}

fn private_path_values(paths: &AppPaths) -> Vec<String> {
    let mut values = vec![
        paths.app_data().to_string_lossy().into_owned(),
        paths.storage_root().to_string_lossy().into_owned(),
    ];
    for variable in ["USERPROFILE", "HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            values.push(value.to_string_lossy().into_owned());
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn sanitize_event(
    event: Event<'static>,
    replacements: &[String],
    installation_id: Option<String>,
) -> Event<'static> {
    let mut value = match serde_json::to_value(&event) {
        Ok(value) => value,
        Err(_) => return minimal_event(event, installation_id),
    };
    sanitize_json_strings(&mut value, replacements);
    let mut event = serde_json::from_value(value)
        .unwrap_or_else(|_| minimal_event(event, installation_id.clone()));
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
    event.user = installation_id.map(|id| User {
        id: Some(id),
        ..User::default()
    });
    event
}

fn minimal_event(event: Event<'static>, installation_id: Option<String>) -> Event<'static> {
    let mut sanitized = Event::new();
    sanitized.event_id = event.event_id;
    sanitized.level = event.level;
    sanitized.release = event.release;
    sanitized.environment = event.environment;
    sanitized.platform = event.platform;
    sanitized.message = Some("slate captured an error that could not be sanitized".to_owned());
    sanitized.user = installation_id.map(|id| User {
        id: Some(id),
        ..User::default()
    });
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

#[cfg(test)]
mod tests {
    use super::sanitize_event;
    use sentry::protocol::{Breadcrumb, Event, Request};

    #[test]
    fn crash_events_remove_requests_paths_and_breadcrumb_fields() {
        let mut event = Event::new();
        event.message = Some("failed in C:\\Users\\player\\Slate access_token=visible".to_owned());
        event.request = Some(Request::default());
        event.breadcrumbs.values.push(Breadcrumb {
            message: Some("opened C:\\Users\\player\\Slate".to_owned()),
            data: [("instance".to_owned(), "private-pack".into())]
                .into_iter()
                .collect(),
            ..Breadcrumb::default()
        });

        let event = sanitize_event(
            event,
            &["C:\\Users\\player".to_owned()],
            Some("anonymous-installation".to_owned()),
        );

        assert!(event.request.is_none());
        assert_eq!(
            event.user.and_then(|user| user.id).as_deref(),
            Some("anonymous-installation")
        );
        assert!(event.message.is_some_and(|message| {
            !message.contains("player") && !message.contains("visible")
        }));
        assert!(event.breadcrumbs.values[0].data.is_empty());
    }
}
