import { ArrowDownToLine, Eraser, Terminal } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { subscribeSessionLog } from "../lib/bridge";
import { formatMinecraftSessionLog } from "../lib/sessionLog";
import type { GameSession, SessionLogEvent } from "../types/launcher";

const MAX_RENDERED_LOG_CHARACTERS = 250_000;

type StreamStatus = "connecting" | "live" | "closed" | "error";

function boundSessionLogText(value: string): string {
  if (value.length <= MAX_RENDERED_LOG_CHARACTERS) return value;
  const minimumStart = value.length - MAX_RENDERED_LOG_CHARACTERS;
  const nextLine = value.indexOf("\n", minimumStart);
  return value.slice(nextLine === -1 ? minimumStart : nextLine + 1);
}

export function SessionLogPanel({
  session,
  active,
  onAttached,
}: {
  session: GameSession;
  active: boolean;
  onAttached: (session: GameSession) => void;
}) {
  const [output, setOutput] = useState("");
  const [status, setStatus] = useState<StreamStatus>("connecting");
  const [statusMessage, setStatusMessage] = useState("");
  const [historyTruncated, setHistoryTruncated] = useState(false);
  const [follow, setFollow] = useState(true);
  const outputElement = useRef<HTMLPreElement>(null);
  const { id, instanceId, logName, mode, pid } = session;
  const displayOutput = useMemo(
    () => formatMinecraftSessionLog(output),
    [output],
  );

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => Promise<void>) | undefined;

    const receive = (event: SessionLogEvent) => {
      if (disposed || event.sessionId !== id) return;
      if (event.kind === "snapshot" || event.kind === "reset") {
        setOutput(boundSessionLogText(event.text));
        setHistoryTruncated(event.truncated);
        setStatus("live");
        return;
      }
      if (event.kind === "append") {
        setOutput((current) => boundSessionLogText(current + event.text));
        setStatus("live");
        return;
      }
      if (event.kind === "closed") {
        setStatus("closed");
        return;
      }
      setStatus("error");
      setStatusMessage(event.text);
    };

    void subscribeSessionLog(id, receive)
      .then((stop) => {
        if (disposed) {
          void stop();
        } else {
          unsubscribe = stop;
          onAttached({ id, instanceId, logName, mode, pid, state: "running" });
        }
      })
      .catch(() => {
        if (!disposed) {
          setStatus("error");
          setStatusMessage(
            "Live game output is unavailable. Reopen the instance and try again.",
          );
        }
      });

    return () => {
      disposed = true;
      if (unsubscribe) void unsubscribe();
    };
  }, [id, instanceId, logName, mode, onAttached, pid]);

  useEffect(() => {
    if (!follow || !outputElement.current) return;
    outputElement.current.scrollTop = outputElement.current.scrollHeight;
  }, [follow, output]);

  const displayStatus =
    status === "live" && !active ? "finishing" : status;
  const statusLabel = {
    connecting: "Connecting",
    live: "Live",
    finishing: "Finishing",
    closed: "Ended",
    error: "Unavailable",
  }[displayStatus];

  return (
    <section className="overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
      <header className="flex min-h-15 items-center justify-between gap-4 border-b border-app-separator/55 px-5 py-3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-compact bg-app-raised text-app-accent">
            <Terminal size={16} aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h2 className="m-0 text-[14px] font-bold tracking-[-.015em]">
              Game output
            </h2>
            <p className="mt-0.5 mb-0 overflow-hidden font-mono text-[10px] text-app-muted text-ellipsis whitespace-nowrap">
              Live Minecraft output
            </p>
          </div>
          <span
            className={`ml-1 inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2 py-1 text-[9px] font-bold tracking-[.06em] uppercase ${
              displayStatus === "live"
                ? "border-app-accent/30 bg-app-accent/10 text-app-accent"
                : displayStatus === "error"
                  ? "border-app-danger/35 bg-app-danger/10 text-app-danger"
                  : "border-app-separator bg-app-bg text-app-secondary"
            }`}
            aria-live="polite"
          >
            <span
              className={`size-1.5 rounded-full ${
                displayStatus === "live"
                  ? "bg-app-accent"
                  : displayStatus === "error"
                    ? "bg-app-danger"
                    : "bg-app-muted"
              }`}
              aria-hidden="true"
            />
            {statusLabel}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            className={`inline-flex h-8 items-center gap-1.5 rounded-compact border px-2.5 text-[10px] font-bold ${
              follow
                ? "border-app-accent/35 bg-app-accent/10 text-app-accent"
                : "border-app-separator bg-app-bg text-app-secondary hover:text-app-text"
            }`}
            aria-pressed={follow}
            onClick={() => {
              setFollow(true);
              if (outputElement.current) {
                outputElement.current.scrollTop =
                  outputElement.current.scrollHeight;
              }
            }}
          >
            <ArrowDownToLine size={13} aria-hidden="true" />
            Follow
          </button>
          <button
            type="button"
            className="inline-flex h-8 items-center gap-1.5 rounded-compact border border-app-separator bg-app-bg px-2.5 text-[10px] font-bold text-app-secondary hover:text-app-text disabled:opacity-45"
            disabled={!output}
            onClick={() => setOutput("")}
          >
            <Eraser size={13} aria-hidden="true" />
            Clear
          </button>
        </div>
      </header>
      {statusMessage ? (
        <p className="m-0 border-b border-app-danger/30 bg-app-danger/8 px-5 py-2.5 text-[11px]/[17px] text-app-danger">
          {statusMessage}
        </p>
      ) : null}
      {historyTruncated ? (
        <p className="m-0 border-b border-app-separator/55 bg-app-raised/45 px-5 py-2 font-mono text-[9px] text-app-muted">
          Earlier output is still in the session log file. Showing the latest
          256 KB.
        </p>
      ) : null}
      <pre
        ref={outputElement}
        role="log"
        aria-label={`Minecraft output for ${session.logName}`}
        aria-live="off"
        tabIndex={0}
        className="m-0 h-64 overflow-auto bg-app-bg p-4 font-mono text-[10px]/[16px] break-words whitespace-pre-wrap text-app-secondary outline-none focus-visible:ring-1 focus-visible:ring-app-accent focus-visible:ring-inset"
        onScroll={(event) => {
          const element = event.currentTarget;
          const atBottom =
            element.scrollHeight - element.scrollTop - element.clientHeight < 24;
          setFollow((current) => (current === atBottom ? current : atBottom));
        }}
      >
        {displayOutput ||
          (output
            ? "Waiting for the current structured log event to finish…"
            : status === "connecting"
              ? "Waiting for Minecraft output…"
              : "No output was written for this session.")}
      </pre>
    </section>
  );
}
