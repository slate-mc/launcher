import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { checkModpackUpdate } from "./bridgeContent";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./bridgeRuntime", () => ({
  bridgeMode: "native",
  requireNativeContent: vi.fn(),
}));

describe("modpack update checks", () => {
  beforeEach(() => vi.clearAllMocks());

  it("accepts an up-to-date native response with null version details", async () => {
    const response = {
      updateAvailable: false,
      currentVersionId: "9675777",
      currentVersionName: null,
      latestVersionId: null,
      latestVersionName: null,
    };
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(checkModpackUpdate("instance-id")).resolves.toEqual(response);
    expect(invoke).toHaveBeenCalledWith("modpack_update_check", {
      request: { instanceId: "instance-id" },
    });
  });

  it("preserves the installed and available version details", async () => {
    const response = {
      updateAvailable: true,
      currentVersionId: "8854298",
      currentVersionName: "ATM10SKY-2.0.4",
      latestVersionId: "9676315",
      latestVersionName: "ATM10SKY-2.4.0",
    };
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(checkModpackUpdate("instance-id")).resolves.toEqual(response);
  });
});
