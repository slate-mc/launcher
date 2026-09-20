import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ProviderMarkdown } from "./ProviderMarkdown";

describe("ProviderMarkdown", () => {
  it("renders provider formatting while removing active HTML", () => {
    const { container } = render(
      <ProviderMarkdown
        value={[
          "## Pack features",
          "",
          "- **Machines** and quests",
          "",
          '<script data-testid="unsafe-script">window.bad = true</script>',
          '<iframe src="https://example.com"></iframe>',
        ].join("\n")}
      />,
    );

    expect(
      screen.getByRole("heading", { name: "Pack features" }),
    ).toBeVisible();
    expect(screen.getByText("Machines")).toHaveProperty("tagName", "STRONG");
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("iframe")).toBeNull();
  });
});
