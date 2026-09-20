import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { App } from "../../app/App";

describe("saved servers", () => {
  beforeAll(() => {
    window.localStorage.removeItem("slate.preview.servers.v1");
    window.history.replaceState({}, "", "/servers");
  });

  it("lists saved servers and opens the inline add flow", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Saved servers" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Blockhaven SMP")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Add server" }));
    expect(
      await screen.findByRole("heading", { name: "Add a server" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveFocus();
  });
});
