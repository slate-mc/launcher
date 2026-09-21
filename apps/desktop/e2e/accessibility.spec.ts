import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

const previewInstanceId = "98d7fe64-acb5-454c-a7e5-7127a3af0c12";

async function openPreview(page: Page, path: string) {
  await page.addInitScript(() => {
    localStorage.clear();
    localStorage.setItem(
      "slate.preview.preferences.v1",
      JSON.stringify({
        theme: "dark",
        downloadConcurrency: 4,
        downloadBandwidthLimitMib: 0,
        telemetryEnabled: false,
        crashReportingEnabled: false,
        updateChannel: "stable",
        reduceMotion: "on",
        trashRetentionDays: 30,
      }),
    );
  });
  await page.goto(path);
  if (path !== "/onboarding") {
    await expect(
      page.getByText(/Development preview · sample data/),
    ).toBeVisible();
  }
}

for (const route of [
  { path: "/home", heading: "Your instances" },
  { path: "/discover", heading: "Find your next modpack" },
  { path: "/onboarding", heading: "Connect the account you play with" },
  { path: "/library", heading: "Your instances" },
  { path: "/servers", heading: "Saved servers" },
  { path: "/downloads", heading: "Installation queue" },
  { path: "/activity", heading: "Running games" },
  { path: "/accounts", heading: "Minecraft accounts" },
  { path: "/help", heading: "Launcher support" },
  { path: "/settings/storage", heading: "Storage" },
  { path: "/settings/general", heading: "Launcher preferences" },
  { path: "/settings/privacy", heading: "Help improve slate" },
  { path: "/library/new", heading: "Build a clean setup" },
  {
    path: `/instances/${previewInstanceId}/overview`,
    heading: "Survival",
  },
  {
    path: `/instances/${previewInstanceId}/content`,
    heading: "Survival",
  },
  {
    path: `/instances/${previewInstanceId}/settings`,
    heading: "Survival",
  },
] as const) {
  test(`${route.path} has no detectable WCAG A or AA violations`, async ({
    page,
  }) => {
    await openPreview(page, route.path);
    await expect(
      page.getByRole("heading", { name: route.heading, exact: true }),
    ).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect(
      results.violations.map((violation) => ({
        id: violation.id,
        targets: violation.nodes.flatMap((node) => node.target),
      })),
    ).toEqual([]);
  });
}

test("destructive confirmation traps focus and restores it when dismissed", async ({
  page,
}) => {
  await openPreview(page, `/instances/${previewInstanceId}/overview`);
  await expect(page.getByRole("heading", { name: "Survival" })).toBeVisible();

  const trigger = page.getByRole("button", { name: "Move to trash" });
  await trigger.click();

  const dialog = page.getByRole("alertdialog", {
    name: "Move Survival to trash?",
  });
  await expect(dialog).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel" })).toBeFocused();

  await page.keyboard.press("Tab");
  await expect(
    dialog.getByRole("button", { name: "Move to trash", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Cancel" })).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
});
