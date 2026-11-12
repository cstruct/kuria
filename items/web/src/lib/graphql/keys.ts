import type { KeyingConfig } from '@urql/exchange-graphcache';

/** What the introspection says of one type. */
interface TypeShape {
	kind: string;
	name: string;
	fields?: { name: string }[] | null;
}

/** Types identified by something other than an `id` field. */
const KEYED: KeyingConfig = {
	AktType: (data) => `${data.key}:${data.version}`
};

/**
 * Graphcache's keys for every object type of the schema: `id` where a type
 * has one, the overrides above, and none (embedded in their parent) for
 * values such as field values, board columns and revisions.
 */
export function keysOf(types: readonly TypeShape[], roots: readonly string[]): KeyingConfig {
	const keys: KeyingConfig = { ...KEYED };
	for (const type of types) {
		if (type.kind !== 'OBJECT' || type.name.startsWith('__') || roots.includes(type.name)) {
			continue;
		}
		if (type.name in keys) continue;
		if (!type.fields?.some((field) => field.name === 'id')) {
			keys[type.name] = () => null;
		}
	}
	return keys;
}
