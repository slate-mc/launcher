import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { App } from "./App";

describe("launcher shell", () => {
  beforeAll(() => {
    window.history.replaceState({}, "", "/home");
  });

  it("labels preview data and renders the primary launcher task", async () => {
    render(<App />);

    expect(
      await screen.findByText(/Development preview · sample data/i),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { name: "Survival" }),
    ).toBeInTheDocument();
    expect(screen.getByTitle("Downloads")).toHaveAttribute("href", "/downloads");
    expect(screen.getByTitle("Activity")).toHaveAttribute("href", "/activity");
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
  });
});
