# Privacy

slate keeps usage analytics and crash reporting separate. Both are off by default and can be
changed independently in Settings > Privacy.

## Anonymous usage events

When enabled, slate records a small allowlisted set of lifecycle events for setup, installation,
content changes, and game launch outcomes. Events may contain the launcher version, operating
system, loader type, provider type, and a random installation ID. They do not contain Microsoft or
Minecraft account details, instance names, server addresses, file paths, worlds, screenshots,
chat, or game logs.

Events wait in a bounded local queue while offline. Turning usage sharing off deletes queued events
and prevents new events from being recorded. Delivery goes through the slate API; the desktop app
does not contain PostHog credentials.

## Crash reports

When enabled, native launcher panics, renderer failures, and selected internal errors may be sent to
Sentry. Reports include the slate release, platform/runtime context, sanitized error text, and stack
frames. A random installation ID allows counting affected installations.

Before transmission, slate removes account and request data, absolute paths, instance context,
breadcrumb fields, local variables, host names, and values shaped like credentials or access
tokens. Minecraft game logs are not attached. Crash reports are not stored in a launcher-side
offline queue.

## Server observability

The slate API records operational telemetry needed to run the service, including normalized route
latency, response status, upstream health, cache outcomes, and server errors. Route templates are
used instead of user-supplied project/version values. Credentials and request bodies are not added
to telemetry.

## Support reports

Support reports are created only when requested by the user. The preview shows what will be
included. Exported reports sanitize credentials, player identity, absolute paths, and other private
values before writing the archive. Private upload is not yet enabled; the current flow saves the
reviewed archive locally.
