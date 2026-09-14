const EVENT_OPEN = "<log4j:Event";
const EVENT_CLOSE = "</log4j:Event>";

export function formatMinecraftSessionLog(input: string): string {
  if (!input.includes(EVENT_OPEN) && !input.includes(EVENT_CLOSE)) return input;

  const output: string[] = [];
  let cursor = 0;
  while (cursor < input.length) {
    const eventStart = input.indexOf(EVENT_OPEN, cursor);
    if (eventStart === -1) {
      appendPlainText(output, input.slice(cursor));
      break;
    }

    appendPlainText(output, input.slice(cursor, eventStart));
    const eventEnd = input.indexOf(EVENT_CLOSE, eventStart);
    if (eventEnd === -1) break;

    const end = eventEnd + EVENT_CLOSE.length;
    output.push(formatLog4jEvent(input.slice(eventStart, end)));
    cursor = end;
  }

  return output.filter(Boolean).join("\n");
}

function appendPlainText(output: string[], source: string) {
  let value = source;
  const orphanEnd = value.lastIndexOf(EVENT_CLOSE);
  if (orphanEnd !== -1) {
    value = value.slice(orphanEnd + EVENT_CLOSE.length);
  }
  const trimmed = value.trim();
  if (trimmed) output.push(trimmed);
}

function formatLog4jEvent(event: string): string {
  const timestamp = formatTimestamp(readAttribute(event, "timestamp"));
  const thread = readAttribute(event, "thread") || "unknown";
  const level = (readAttribute(event, "level") || "info").toUpperCase();
  const logger = readAttribute(event, "logger");
  const message = readElementText(event, "Message").trimEnd();
  const throwable = readElementText(event, "Throwable").trim();
  const source = logger ? ` [${logger}]` : "";
  const body = message || "(empty log event)";
  return `[${timestamp}] [${thread}/${level}]${source}: ${body}${
    throwable ? `\n${throwable}` : ""
  }`;
}

function readAttribute(event: string, name: string): string {
  const match = event.match(new RegExp(`\\s${name}="([^"]*)"`));
  return match ? decodeXml(match[1]) : "";
}

function readElementText(event: string, name: string): string {
  const match = event.match(
    new RegExp(
      `<log4j:${name}(?:\\s[^>]*)?>\\s*(?:<!\\[CDATA\\[([\\s\\S]*?)\\]\\]>|([\\s\\S]*?))\\s*</log4j:${name}>`,
    ),
  );
  if (!match) return "";
  return match[1] ?? decodeXml(match[2] ?? "");
}

function decodeXml(value: string): string {
  return value
    .replace(/&#(x?)([0-9a-f]+);/gi, (entity, hexadecimal, digits) => {
      const codePoint = Number.parseInt(digits, hexadecimal ? 16 : 10);
      if (!Number.isSafeInteger(codePoint) || codePoint > 0x10ffff) {
        return entity;
      }
      return String.fromCodePoint(codePoint);
    })
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

function formatTimestamp(value: string): string {
  const milliseconds = Number(value);
  if (!Number.isFinite(milliseconds)) return "--:--:--";
  const date = new Date(milliseconds);
  if (Number.isNaN(date.getTime())) return "--:--:--";
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((part) => part.toString().padStart(2, "0"))
    .join(":");
}
