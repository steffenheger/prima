import { expect, test } from '@playwright/test';

test('Set company data 1', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: 'Login' })).toBeVisible();
	await page.getByLabel('Email').fill('taxi@test2.de');
	await page.getByLabel('Password').fill('blabla');
	await page.getByRole('button', { name: 'Login' }).click();
	expect(page).toHaveURL('/company');
	await expect(page.getByRole('heading', { name: 'Stammdaten ihres Unternehmens' })).toBeVisible();
	// Login succeeded
	await page.getByLabel('Name').fill('Test');
	await page.getByRole('button', { name: 'Übernehmen' }).click();

	await expect(page.getByText('Die Eingabe muss mindestens 2 Zeichen enthalten.')).toBeVisible();
	await expect(page.getByText('Es wurde kein Gebiet ausgewählt.')).toBeVisible();
	await expect(page.getByText('Es wurde keine Gemeinde ausgewählt.')).toBeVisible();
});

test('Set company data 2', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: 'Login' })).toBeVisible();
	await page.getByLabel('Email').fill('taxi@test2.de');
	await page.getByLabel('Password').fill('blabla');
	await page.getByRole('button', { name: 'Login' }).click();
	expect(page).toHaveURL('/company');
	await expect(page.getByRole('heading', { name: 'Stammdaten ihres Unternehmens' })).toBeVisible();
	// Login succeeded
	await page.getByLabel('Name').fill('Test');
	await page.getByLabel('Unternehmenssitz').fill('Plantagenweg 3, 02827 Görlitz');
	await page.getByRole('button', { name: 'Übernehmen' }).click();

	await expect(page.getByText('Die Eingabe muss mindestens 2 Zeichen enthalten.')).not.toBeVisible();
	await expect(page.getByText('Es wurde kein Gebiet ausgewählt.')).toBeVisible();
	await expect(page.getByText('Es wurde keine Gemeinde ausgewählt.')).toBeVisible();
	// await expect(page.getByText('Die Daten wurden übernommen.')).not.toBeVisible();
});

test('Set company data 3', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: 'Login' })).toBeVisible();
	await page.getByLabel('Email').fill('taxi@test2.de');
	await page.getByLabel('Password').fill('blabla');
	await page.getByRole('button', { name: 'Login' }).click();
	expect(page).toHaveURL('/company');
	await expect(page.getByRole('heading', { name: 'Stammdaten ihres Unternehmens' })).toBeVisible();
	// Login succeeded
	await page.getByLabel('Name').fill('Test');
	await page.getByLabel('Unternehmenssitz').fill('Plantagenweg 3, 02827 Görlitz');
	await page.getByTestId('select-zone').click();
	await page.getByText('Görlitz').click();
	await page.getByRole('button', { name: 'Übernehmen' }).click();

	await expect(page.getByText('Die Eingabe muss mindestens 2 Zeichen enthalten.')).not.toBeVisible();
	await expect(page.getByText('Es wurde kein Gebiet ausgewählt.')).not.toBeVisible();
	await expect(page.getByText('Es wurde keine Gemeinde ausgewählt.')).toBeVisible();
	// await expect(page.getByText('Die Daten wurden übernommen.')).not.toBeVisible();
});

test('Set company data 4', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: 'Login' })).toBeVisible();
	await page.getByLabel('Email').fill('taxi@test2.de');
	await page.getByLabel('Password').fill('blabla');
	await page.getByRole('button', { name: 'Login' }).click();
	expect(page).toHaveURL('/company');
	await expect(page.getByRole('heading', { name: 'Stammdaten ihres Unternehmens' })).toBeVisible();
	// Login succeeded
	await page.getByLabel('Name').fill('Test');
	await page.getByLabel('Unternehmenssitz').fill('Plantagenweg 3, 02827 Görlitz');
	await page.getByTestId('select-zone').click();
	await page.getByText('Görlitz').click();
	await page.getByTestId('select-community').click();
	await page.getByText('Weißwasser/O.L.').click();
	await page.getByRole('button', { name: 'Übernehmen' }).click();

	await expect(page.getByText('Die Eingabe muss mindestens 2 Zeichen enthalten.')).not.toBeVisible();
	await expect(page.getByText('Es wurde kein Gebiet ausgewählt.')).not.toBeVisible();
	await expect(page.getByText('Es wurde keine Gemeinde ausgewählt.')).not.toBeVisible();
	await expect(page.getByTestId('community-error')).toHaveText('Die Addresse liegt nicht in der ausgewählten Gemeinde.');
	// await expect(page.getByText('Die Daten wurden übernommen.')).not.toBeVisible();
});
