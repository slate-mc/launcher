import { Channel, invoke } from "@tauri-apps/api/core";
import {
  gameSessionListSchema,
  gameSessionSchema,
  sessionLogEventSchema,
  sessionLogSubscriptionSchema,
  type GameSession,
  type SessionLogEvent,
} from "../types/launcher";
import { bridgeMode } from "./bridgeRuntime";

export async function launchInstance(
  id: string,
  accountId?: string,
  serverAddress?: string,
): Promise<GameSession> {
  if (bridgeMode === "native") {
    return gameSessionSchema.parse(
      await invoke("instance_launch", {
        request: { id, accountId, serverAddress },
      }),
    );
  }
  throw new Error(
    "Minecraft launch is available only in the slate desktop app.",
  );
}

export async function listGameSessions(): Promise<GameSession[]> {
  if (bridgeMode === "native") {
    return gameSessionListSchema.parse(await invoke("sessions_list"));
  }
  return [];
}

export async function forceStopGameSession(id: string): Promise<GameSession> {
  if (bridgeMode === "native") {
    return gameSessionSchema.parse(
      await invoke("session_force_stop", { request: { id } }),
    );
  }
  throw new Error(
    "Minecraft process control is available only in the slate desktop app.",
  );
}

export async function subscribeSessionLog(
  sessionId: string,
  onEvent: (event: SessionLogEvent) => void,
): Promise<() => Promise<void>> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Live Minecraft logs are available only in the slate desktop app.",
    );
  }
  const channel = new Channel<unknown>();
  channel.onmessage = (value) => onEvent(sessionLogEventSchema.parse(value));
  const subscription = sessionLogSubscriptionSchema.parse(
    await invoke("session_log_subscribe", {
      request: { sessionId },
      onEvent: channel,
    }),
  );
  return async () => {
    await invoke("session_log_unsubscribe", {
      request: { subscriptionId: subscription.id },
    });
  };
}
