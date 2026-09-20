import { invoke } from "@tauri-apps/api/core";
import {
  authFlowStatusSchema,
  authStartSchema,
  minecraftAccountListSchema,
  minecraftAccountSchema,
  type AuthFlowStatus,
  type AuthStart,
  type MinecraftAccount,
} from "../types/launcher";
import { bridgeMode } from "./bridgeRuntime";

export async function listAccounts(): Promise<MinecraftAccount[]> {
  if (bridgeMode === "native") {
    return minecraftAccountListSchema.parse(await invoke("accounts_list"));
  }
  return [];
}

export async function startMinecraftAuth(): Promise<AuthStart> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Microsoft sign-in is available only in the slate desktop app.",
    );
  }
  return authStartSchema.parse(await invoke("auth_start"));
}

export async function getMinecraftAuthStatus(
  flowId: string,
): Promise<AuthFlowStatus> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Microsoft sign-in is available only in the slate desktop app.",
    );
  }
  return authFlowStatusSchema.parse(
    await invoke("auth_get_status", { flowId }),
  );
}

export async function cancelMinecraftAuth(
  flowId: string,
): Promise<AuthFlowStatus> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Microsoft sign-in is available only in the slate desktop app.",
    );
  }
  return authFlowStatusSchema.parse(
    await invoke("auth_cancel", { request: { flowId } }),
  );
}

export async function refreshMinecraftAccount(
  id: string,
): Promise<MinecraftAccount> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Account refresh is available only in the slate desktop app.",
    );
  }
  return minecraftAccountSchema.parse(
    await invoke("account_refresh", { request: { id } }),
  );
}

export async function setDefaultMinecraftAccount(
  id: string,
): Promise<MinecraftAccount> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Account selection is available only in the slate desktop app.",
    );
  }
  return minecraftAccountSchema.parse(
    await invoke("account_set_default", { request: { id } }),
  );
}

export async function removeMinecraftAccount(id: string): Promise<void> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Account removal is available only in the slate desktop app.",
    );
  }
  await invoke("account_remove", { request: { id } });
}
