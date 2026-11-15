/**
 * The colour pairs the app puts together, and their contrast: what the
 * tokens in tokens.css must meet in light and in dark (WCAG 2.2 level AA:
 * 4.5:1 for text, 3:1 for the boundaries of controls).
 */
// r[impl app.themes]

/** A foreground token, the background token it is used on, and the least
 * contrast it needs. */
export const PAIRS: [fg: string, bg: string, least: number][] = [
	['text', 'bg', 4.5],
	['text', 'bg-subtle', 4.5],
	['text', 'bg-muted', 4.5],
	['text', 'surface', 4.5],
	['text', 'surface-raised', 4.5],
	['text-muted', 'bg', 4.5],
	['text-muted', 'bg-subtle', 4.5],
	['text-muted', 'surface', 4.5],
	['accent', 'bg', 4.5],
	['accent', 'surface', 4.5],
	['on-accent', 'accent', 4.5],
	['on-accent', 'accent-hover', 4.5],
	['on-accent-subtle', 'accent-subtle', 4.5],
	['on-danger', 'danger', 4.5],
	['on-danger', 'danger-hover', 4.5],
	['tone-neutral-fg', 'tone-neutral-bg', 4.5],
	['tone-active-fg', 'tone-active-bg', 4.5],
	['tone-success-fg', 'tone-success-bg', 4.5],
	['tone-warning-fg', 'tone-warning-bg', 4.5],
	['tone-danger-fg', 'tone-danger-bg', 4.5],
	...(['neutral', 'blue', 'green', 'amber', 'red', 'purple', 'teal', 'pink'] as const).map(
		(c): [string, string, number] => [`type-${c}-fg`, `type-${c}-bg`, 4.5]
	),
	// Inputs' borders and the focus ring, against what they sit on.
	['border-strong', 'bg', 3],
	['border-strong', 'surface', 3],
	['focus', 'bg', 3],
	['focus', 'surface', 3],
	// Sheets (app.sheets): the same in either theme.
	['sheet-text', 'sheet-bg', 4.5],
	['sheet-text', 'sheet-subtle', 4.5],
	['sheet-text', 'sheet-code-bg', 4.5],
	['sheet-muted', 'sheet-bg', 4.5],
	['sheet-link', 'sheet-bg', 4.5],
	['sheet-link-hover', 'sheet-bg', 4.5],
	['sheet-visited', 'sheet-bg', 4.5],
	['sheet-danger', 'sheet-bg', 4.5],
	['sheet-border-strong', 'sheet-bg', 3],
	['sheet-focus', 'sheet-bg', 3]
];

function channel(value: number): number {
	const c = value / 255;
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** The relative luminance of a `#rrggbb` colour. */
export function luminance(hex: string): number {
	const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
	return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** The contrast ratio of two `#rrggbb` colours, from 1 to 21. */
export function contrast(a: string, b: string): number {
	const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (light + 0.05) / (dark + 0.05);
}

/** The `--name: #rrggbb` colour tokens declared in a block of CSS. */
export function colours(css: string): Record<string, string> {
	return Object.fromEntries(
		[...css.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6});/gi)].map(([, name, hex]) => [
			name,
			hex.toLowerCase()
		])
	);
}
