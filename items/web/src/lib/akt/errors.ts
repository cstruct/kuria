/** Refusals as the app shows them: a message per code, and the field a
 * refused value belongs to. */
// r[impl app.errors]
import type { CombinedError } from '@urql/core';

export interface Refusal {
	code: string;
	message: string;
	/** The key of the field whose value was refused, if one was. */
	field?: string;
	/** For `BROKEN_REFERENCES`: each broken reference, and why. */
	references?: string[];
	/** For `BROKEN_REFERENCES`: the akter that releasing together would
	 * mend references to (`version.together`). */
	together?: string[];
}

const MESSAGES: Record<string, string> = {
	STALE:
		'Someone changed this while you were working on it. It shows their change now, and what you entered is kept: apply it again.',
	FORBIDDEN: 'You aren’t allowed to do that.',
	NOT_FOUND: 'That doesn’t exist, or you can’t see it.',
	UNKNOWN_AKT_TYPE: 'There is no such akt type, or you can’t see it.',
	NOT_CONTAINABLE: 'That container can’t hold akter of this type.',
	ALREADY_MEMBER: 'It is already in that container.',
	NOT_MEMBER: 'It isn’t in that container.',
	EXCLUSIVE: 'It can be in only one container of that type.',
	CYCLE: 'An akt can’t be inside itself.',
	FROZEN: 'That container is in a final state, so its members can’t change.',
	NOT_ACCEPTING: 'That container doesn’t take akter of this type in its current state.',
	CREATED_IN: 'Akter of this type must be created inside a container.',
	UNAVAILABLE: 'Kuria can’t check your permissions right now. Try again.',
	INVALID_CONTACT: 'A contact needs a name, and a real email if any.',
	DUPLICATE_EMAIL: 'Another contact has that email.',
	LINKED: 'That contact is linked to a user, so it can’t be edited.',
	// r[impl app.archive]
	ARCHIVED: 'It is archived, so it can’t be changed. Unarchive it first.',
	BROKEN_REFERENCES: 'It can’t be released while a reference is broken.',
	INTERNAL: 'Something went wrong in Kuria. Try again.'
};

/** The refusal `error` carries, with field names from `fieldNames`. */
export function refusalOf(error: CombinedError, fieldNames: Record<string, string> = {}): Refusal {
	const first = error.graphQLErrors[0];
	if (!first) {
		return { code: 'NETWORK', message: 'Kuria can’t be reached. Check your connection.' };
	}
	const code = String(first.extensions?.code ?? 'INTERNAL');
	// The server names a refused value's field first: "`root_cause`: …".
	const named = /^`([a-z][a-z0-9_]*)`/.exec(first.message);
	if (code === 'REFUSED' && named) {
		const key = named[1];
		const reason = first.message.slice(named[0].length).replace(/^[:\s]+/, '');
		const name = fieldNames[key] ?? key;
		return {
			code,
			field: key,
			message: reason.startsWith('is required') ? `${name} is required.` : `${name}: ${reason}`
		};
	}
	if (code === 'BROKEN_REFERENCES') {
		const { references, together } = first.extensions ?? {};
		return {
			code,
			message: MESSAGES[code],
			references: Array.isArray(references) ? references.map(String) : [],
			together: Array.isArray(together) ? together.map(String) : []
		};
	}
	return { code, message: MESSAGES[code] ?? first.message };
}
