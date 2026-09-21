import * as Sentry from "@sentry/react";
import { getVersion } from "@tauri-apps/api/app";

let initialized = false;

export async function configureBrowserCrashReporting(enabled: boolean) {
  const dsn = import.meta.env.VITE_SENTRY_DSN?.trim();
  if (!dsn) return;

  if (!initialized) {
    const version = await getVersion().catch(() => "development");
    Sentry.init({
      dsn,
      enabled,
      release: `slate-desktop@${version}`,
      environment: import.meta.env.DEV ? "development" : "production",
      sendDefaultPii: false,
      tracesSampleRate: 0,
      beforeSend: sanitizeEvent,
    });
    initialized = true;
    return;
  }

  const client = Sentry.getClient();
  if (client) client.getOptions().enabled = enabled;
}

export function sanitizeEvent(event: Sentry.ErrorEvent): Sentry.ErrorEvent {
  delete event.user;
  delete event.request;
  delete event.server_name;
  delete event.extra;
  delete event.tags;
  event.breadcrumbs = [];
  event.contexts = retainSafeContexts(event.contexts);
  if (event.message) event.message = sanitizeText(event.message);
  for (const exception of event.exception?.values ?? []) {
    if (exception.value) exception.value = sanitizeText(exception.value);
    for (const frame of exception.stacktrace?.frames ?? []) {
      delete frame.abs_path;
      delete frame.vars;
    }
  }
  return event;
}

function retainSafeContexts(contexts: Sentry.ErrorEvent["contexts"]) {
  if (!contexts) return undefined;
  return Object.fromEntries(
    Object.entries(contexts).filter(([key]) =>
      ["browser", "device", "os", "runtime"].includes(key),
    ),
  );
}

function sanitizeText(value: string) {
  let sanitized = value
    .replace(/[A-Za-z]:[\\/](?:[^\\/\s]+[\\/])*[^\\/\s]*/g, "<private-path>")
    .replace(/\/(?:home|Users)\/[^\s"'&]+/g, "<private-path>");
  for (const marker of [
    "access_token=",
    "refresh_token=",
    "client_secret=",
    "authorization=",
    "Bearer ",
  ]) {
    sanitized = redactMarkerValue(sanitized, marker);
  }
  return sanitized;
}

function redactMarkerValue(value: string, marker: string) {
  let remaining = value;
  let output = "";
  for (;;) {
    const index = remaining.indexOf(marker);
    if (index < 0) return output + remaining;
    const valueStart = index + marker.length;
    output += `${remaining.slice(0, valueStart)}[redacted]`;
    const tail = remaining.slice(valueStart);
    const valueEnd = tail.search(/[\s&"']/);
    remaining = valueEnd < 0 ? "" : tail.slice(valueEnd);
  }
}
