import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { InstallProgressIndicator } from "./InstallProgressIndicator";

describe("InstallProgressIndicator", () => {
  it("announces determinate artifact progress", () => {
    render(
      <InstallProgressIndicator
        job={{
          id: "12345678-1234-4234-8234-123456789abc",
          instanceId: "22345678-1234-4234-8234-123456789abc",
          revisionId: "32345678-1234-4234-8234-123456789abc",
          state: "running",
          phase: "assets",
          message: "Checking game assets",
          completedItems: 25,
          totalItems: 100,
          createdAt: "2026-09-13T00:00:00Z",
          updatedAt: "2026-09-13T00:00:01Z",
        }}
      />,
    );

    expect(screen.getByText("Game assets")).toBeInTheDocument();
    expect(screen.getByText("25 / 100 files")).toBeInTheDocument();
    expect(screen.getByText("Checking game assets")).toBeVisible();
    const progress = screen.getByRole("progressbar", {
      name: "Checking game assets",
    });
    expect(progress).toHaveAttribute("aria-valuenow", "25");
    expect(progress).toHaveAttribute("aria-valuetext", "25%");
  });

  it("announces opaque installer work without a fabricated percentage", () => {
    render(
      <InstallProgressIndicator
        job={{
          id: "12345678-1234-4234-8234-123456789abc",
          instanceId: "22345678-1234-4234-8234-123456789abc",
          revisionId: "32345678-1234-4234-8234-123456789abc",
          state: "running",
          phase: "loader",
          message: "Running the NeoForge client installer",
          createdAt: "2026-09-13T00:00:00Z",
          updatedAt: "2026-09-13T00:00:01Z",
        }}
      />,
    );

    expect(screen.getByText("Working")).toBeInTheDocument();
    expect(
      screen.getByText("Running the NeoForge client installer"),
    ).toBeVisible();
    const progress = screen.getByRole("progressbar", {
      name: "Running the NeoForge client installer",
    });
    expect(progress).not.toHaveAttribute("aria-valuenow");
    expect(progress).toHaveAttribute("aria-valuetext", "In progress");
  });

  it("does not expose a persisted internal failure message", () => {
    render(
      <InstallProgressIndicator
        job={{
          id: "12345678-1234-4234-8234-123456789abc",
          instanceId: "22345678-1234-4234-8234-123456789abc",
          revisionId: "32345678-1234-4234-8234-123456789abc",
          state: "failed",
          phase: "content",
          message: "request failed for https://private.example/C:/Users/name/file.jar",
          createdAt: "2026-09-13T00:00:00Z",
          updatedAt: "2026-09-13T00:00:01Z",
        }}
      />,
    );

    expect(
      screen.getByText("Installation did not complete. Retry or repair the instance."),
    ).toBeVisible();
    expect(screen.queryByText(/private\.example/)).not.toBeInTheDocument();
  });
});
