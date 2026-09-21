import { expect, test, type Page } from "@playwright/test";

type Theme = "dark" | "light";

async function openPreview(page: Page, theme: Theme) {
  await page.addInitScript((selectedTheme) => {
    localStorage.clear();
    localStorage.setItem(
      "slate.preview.preferences.v1",
      JSON.stringify({
        theme: selectedTheme,
        downloadConcurrency: 4,
        downloadBandwidthLimitMib: 0,
        telemetryEnabled: false,
        reduceMotion: "on",
        trashRetentionDays: 30,
      }),
    );
  }, theme);
  await page.goto("/home");
  await expect(page.getByRole("status")).toContainText("Development preview");
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
  await expect(
    page.getByRole("heading", { name: "Your instances" }),
  ).toBeVisible();
}

for (const theme of ["dark", "light"] as const) {
  test(`${theme} home shell stays visually stable`, async ({ page }) => {
    await openPreview(page, theme);
    await expect(page).toHaveScreenshot(`home-${theme}.png`, {
      fullPage: true,
    });
  });
}

test("primary navigation remains usable at the minimum window size", async ({
  page,
}) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await openPreview(page, "dark");

  await page.getByRole("link", { name: "Library" }).click();
  await expect(page).toHaveURL(/\/library$/);
  await expect(
    page.getByRole("heading", { name: "Your instances" }),
  ).toBeVisible();

  await page.getByRole("link", { name: "Settings" }).click();
  await expect(page).toHaveURL(/\/settings\/general$/);
  await expect(
    page.getByRole("heading", { name: "Launcher preferences" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Launcher updates" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Check for updates" }),
  ).toBeDisabled();

  await page.getByRole("link", { name: "Privacy" }).click();
  await expect(page).toHaveURL(/\/settings\/privacy$/);
  await expect(
    page.getByRole("heading", { name: "Help improve slate" }),
  ).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Share usage" })).not.toBeChecked();
});
