// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			/** The akter followed to this page (`app.trail`). */
			trail?: import('#lib/shell/trail.ts').Step[];
		}
		// interface Platform {}
	}
}

export {};
