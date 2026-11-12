import { defineConfig, devices } from '@playwright/test';
import { KURIA } from './support/stack.ts';

export default defineConfig({
	testDir: 'tests',
	// One Kuria and one Konklav, shared: the tests run one at a time.
	workers: 1,
	fullyParallel: false,
	timeout: 120_000,
	expect: { timeout: 15_000 },
	reporter: [['list'], ['html', { open: 'never', outputFolder: '.state/report' }]],
	outputDir: '.state/results',
	use: {
		baseURL: KURIA,
		// A control that isn't there fails a step in seconds, not at the end.
		actionTimeout: 15_000,
		navigationTimeout: 30_000,
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure'
	},
	projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
	webServer: {
		command: 'node scripts/serve.ts',
		url: `${KURIA}/healthz`,
		// The first run compiles the server.
		timeout: 900_000,
		reuseExistingServer: false,
		stdout: 'pipe',
		stderr: 'pipe'
	}
});
