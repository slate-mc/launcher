import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "../../app/App";

describe("instance content", () => {
  afterEach(cleanup);

  beforeEach(() => {
    window.history.replaceState(
      {},
      "",
      "/instances/98d7fe64-acb5-454c-a7e5-7127a3af0c12/content",
    );
  });

  it("locks mod discovery to the selected instance target", async () => {
    render(<App />);

    expect(
      await screen.findByRole(
        "heading",
        { name: "Installed mods" },
        { timeout: 3_000 },
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add mods" }));

    expect(
      await screen.findByText("Showing compatible mods"),
    ).toBeInTheDocument();
    expect(
      screen.getAllByText(/Minecraft 1\.21\.1 · Fabric 0\.16\.10/),
    ).toHaveLength(2);
    expect(
      await screen.findByRole("heading", { name: "No compatible mods found" }),
    ).toBeInTheDocument();
  });

  it("offers local ZIP import and requires a world for data packs", async () => {
    render(<App />);

    fireEvent.click(
      await screen.findByRole(
        "button",
        { name: /Resource packs/ },
        { timeout: 3_000 },
      ),
    );
    expect(
      await screen.findByRole("heading", { name: "Resource packs" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Import ZIP" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Data packs/ }));
    expect(
      await screen.findByRole("heading", { name: "Data packs" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Import ZIP" })).toBeDisabled();
    expect(screen.getByText("No worlds found")).toBeInTheDocument();
  });
});
