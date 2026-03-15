const { test, expect } = require('playwright/test');

test('dashboard and wiki render live dx-terminal data', async ({ page }) => {
  const consoleErrors = [];
  page.on('pageerror', err => consoleErrors.push(`pageerror:${err.message}`));
  page.on('console', msg => {
    if (msg.type() === 'error') {
      consoleErrors.push(`console:${msg.text()}`);
    }
  });

  await page.goto('http://127.0.0.1:3310/', { waitUntil: 'networkidle' });
  await page.waitForTimeout(1500);

  await expect(page.locator('#vision-section')).toBeVisible();
  await expect(page.locator('#vision-project')).toHaveText(/dx-terminal/);
  await expect(page.locator('#vision-mission')).not.toHaveText(/^$/);
  await expect(page.locator('#wiki-link')).toHaveAttribute('href', /\/wiki\?project=dx-terminal/);
  await expect(page.locator('text=Guidance Docs')).toBeVisible();
  await expect(page.locator('text=Research & Discovery')).toBeVisible();
  await expect(page.locator('text=AGENTS.md')).toBeVisible();
  await expect(page.locator('text=F9.1 · research')).toBeVisible();

  expect(consoleErrors).toEqual([]);

  await page.goto('http://127.0.0.1:3310/wiki?project=dx-terminal', {
    waitUntil: 'networkidle',
  });
  await expect(page.getByRole('heading', { name: 'Milestones and checkpoints' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Goals' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Features' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Architecture decisions' })).toBeVisible();
});
