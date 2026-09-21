import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { App } from "../../app/App";

describe("support reports", () => {
  beforeAll(() => {
    window.history.replaceState({}, "", "/help");
  });

  it("explains the report contents before anything is shared", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Create a support report" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Never included")).toBeInTheDocument();
    expect(
      screen.getByText("Account credentials or player identity"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Send report" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Save a copy" }),
    ).toBeDisabled();
  });
});
