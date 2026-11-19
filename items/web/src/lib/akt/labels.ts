/** The accessible names of the app's controls that a visible label alone
 * doesn't give, so every control has one: what people with assistive
 * technology hear, and what the end-to-end tests select by. */
// r[impl app.accessible]
import type { FieldDef } from './values';

/** A field editor's group: the field's name, saying if it is required. */
export function legendOf(field: Pick<FieldDef, 'name' | 'required'>): string {
	return field.required ? `${field.name} (required)` : field.name;
}

/** A member's menu, named by the member, since its trigger shows only an
 * ellipsis, and its items, which the menu's name already places. */
export function boardLabels(number: string) {
	return {
		menu: `Actions for ${number}`,
		up: 'Move up',
		down: 'Move down',
		remove: 'Remove from here',
		into: (column: string) => `Move to ${column}`
	};
}
