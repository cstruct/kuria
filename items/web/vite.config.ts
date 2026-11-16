import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

/** The Kuria server the dev server passes the API to: `mise run serve`. */
const server = process.env.KURIA_DEV_SERVER ?? 'http://localhost:8080';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// A single-page app: one fallback page loads every path, which the
			// Kuria server serves for every GET it doesn't route itself.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		// The app must share the server's origin: its session cookie is
		// SameSite=Strict, and the login page reaches Konklav through it.
		proxy: Object.fromEntries(
			['/graphql', '/akter', '/auth', '/attachments', '/v1', '/.well-known', '/healthz'].map(
				(path) => [
					path,
					// Subscriptions, and live drafts (`/akter/{number}/live`), over
					// WebSockets.
					{ target: server, ws: path === '/graphql' || path === '/akter', changeOrigin: false }
				]
			)
		)
	},
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'unit',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
