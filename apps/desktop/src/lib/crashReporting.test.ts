import { describe, expect, it } from "vitest";
import { sanitizeEvent } from "./crashReporting";

describe("crash reporting", () => {
  it("removes identity and secret-bearing event fields", () => {
    const event = sanitizeEvent({
      type: undefined,
      message: "request failed at C:/Users/player/slate Bearer visible",
      user: { id: "player" },
      request: { url: "https://example.test/private" },
      breadcrumbs: [{ message: "private instance", data: { name: "pack" } }],
      exception: {
        values: [
          {
            type: "Error",
            value: "access_token=visible",
            stacktrace: {
              frames: [{ filename: "main.ts", abs_path: "C:/Users/player/main.ts" }],
            },
          },
        ],
      },
    });

    expect(event.user).toBeUndefined();
    expect(event.request).toBeUndefined();
    expect(event.breadcrumbs).toEqual([]);
    expect(event.message).not.toContain("visible");
    expect(event.message).not.toContain("player");
    expect(event.exception?.values?.[0]?.value).not.toContain("visible");
    expect(
      event.exception?.values?.[0]?.stacktrace?.frames?.[0]?.abs_path,
    ).toBeUndefined();
  });
});
