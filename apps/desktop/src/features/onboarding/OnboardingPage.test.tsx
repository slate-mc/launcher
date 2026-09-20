import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { App } from "../../app/App";

describe("first-run setup", () => {
  beforeAll(() => {
    window.history.replaceState({}, "", "/onboarding");
  });

  it("starts with Minecraft account connection and shows the complete setup path", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", {
        name: "Connect the account you play with",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("Storage")).toBeInTheDocument();
    expect(screen.getByText("Java")).toBeInTheDocument();
    expect(screen.getByText("First instance")).toBeInTheDocument();
  });
});
