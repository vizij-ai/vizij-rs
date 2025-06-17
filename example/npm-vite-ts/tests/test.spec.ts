import { test, expect } from '@playwright/test';

test('has title', async ({ page }) => {
  await page.goto('/');

  // Expect the title to match our set title
  await expect(page).toHaveTitle('Example Web App using WASM');
});

test('has body visible', async ({ page }) => {
  await page.goto('/');

  // Check that the page loads successfully
  await expect(page.locator('body')).toBeVisible();
});

test('button click increments counter', async ({ page }) => {
  await page.goto('/');

  // Find the button with the counter
  const buttonLocator = page.locator('button:has-text("count is")');

  // Get the initial count from the button text
  const initialText = await buttonLocator.textContent();
  const initialCount = parseInt(initialText!.match(/\d+/)![0]);

  // Click the button
  await buttonLocator.click();

  // Get the updated count
  const updatedText = await buttonLocator.textContent();
  const updatedCount = parseInt(updatedText!.match(/\d+/)![0]);
  expect(updatedText).toBe('Count is 1. WasmAnimationEngine created successfully!');

  // Verify the count increased by 1
  expect(updatedCount).toBe(initialCount + 1);
});
