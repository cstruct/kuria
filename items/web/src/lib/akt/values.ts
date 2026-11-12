/**
 * Field values in the app: what each field kind's editor holds, the input
 * the server takes for it, and its text. Every kind the server knows is
 * here; no akt type is.
 */
// r[impl app.fields]
import type {
	FieldValuesFragment,
	SomeoneShownFragment,
	TypeShapeFragment
} from '#lib/graphql/generated/graphql.ts';

export type FieldDef = TypeShapeFragment['fields'][number];
export type Value = FieldValuesFragment;

/** A user or a contact, as an input names them. */
export type Who = { user: string } | { contact: string };

/** What a field's editor holds. */
export type Draft =
	| { kind: 'TEXT'; text: string }
	| { kind: 'CHOICE'; key: string }
	| { kind: 'CHOICES'; keys: string[] }
	| { kind: 'DATE'; date: string }
	| { kind: 'USER'; who: Who | null }
	| { kind: 'CHECKBOX'; checked: boolean; for: Who | null }
	| { kind: 'REFERENCE'; url: string; label: string }
	| { kind: 'GIT'; repository: string; sha: string; tag: string }
	| { kind: 'DOCKER'; repository: string; digest: string; tag: string }
	| { kind: 'ATTACHMENT'; id: string | null; name: string | null };

export function whoOf(someone: SomeoneShownFragment): Who {
	return someone.__typename === 'Contact' ? { contact: someone.id } : { user: someone.id };
}

/** The editor's state for `field`, holding `value` if it has one. */
export function draftOf(field: FieldDef, value: Value | undefined): Draft {
	switch (field.kind) {
		case 'TEXT':
			return { kind: 'TEXT', text: value?.__typename === 'TextValue' ? value.text : '' };
		case 'CHOICE':
			return { kind: 'CHOICE', key: value?.__typename === 'ChoiceValue' ? value.option.key : '' };
		case 'CHOICES':
			return {
				kind: 'CHOICES',
				keys: value?.__typename === 'ChoicesValue' ? value.options.map((o) => o.key) : []
			};
		case 'DATE':
			return { kind: 'DATE', date: value?.__typename === 'DateValue' ? value.date : '' };
		case 'USER':
			return { kind: 'USER', who: value?.__typename === 'UserValue' ? whoOf(value.user) : null };
		case 'CHECKBOX': {
			if (value?.__typename !== 'CheckboxValue')
				return { kind: 'CHECKBOX', checked: false, for: null };
			const forWho = whoOf(value.for);
			const self = 'user' in forWho && forWho.user === value.by.id;
			return { kind: 'CHECKBOX', checked: true, for: self ? null : forWho };
		}
		case 'REFERENCE':
			return value?.__typename === 'ReferenceValue'
				? { kind: 'REFERENCE', url: value.url, label: value.label ?? '' }
				: { kind: 'REFERENCE', url: '', label: '' };
		case 'GIT':
			return value?.__typename === 'GitValue'
				? { kind: 'GIT', repository: value.repository, sha: value.sha ?? '', tag: value.tag ?? '' }
				: { kind: 'GIT', repository: '', sha: '', tag: '' };
		case 'DOCKER':
			return value?.__typename === 'DockerValue'
				? {
						kind: 'DOCKER',
						repository: value.repository,
						digest: value.digest,
						tag: value.tag ?? ''
					}
				: { kind: 'DOCKER', repository: '', digest: '', tag: '' };
		case 'ATTACHMENT':
			return value?.__typename === 'AttachmentValue'
				? { kind: 'ATTACHMENT', id: value.attachment.id, name: value.attachment.name }
				: { kind: 'ATTACHMENT', id: null, name: null };
	}
}

/** Whether the editor holds no value. */
export function isEmpty(draft: Draft): boolean {
	switch (draft.kind) {
		case 'TEXT':
			return draft.text === '';
		case 'CHOICE':
			return draft.key === '';
		case 'CHOICES':
			return draft.keys.length === 0;
		case 'DATE':
			return draft.date === '';
		case 'USER':
			return draft.who === null;
		case 'CHECKBOX':
			return !draft.checked;
		case 'REFERENCE':
			return draft.url.trim() === '';
		case 'GIT':
			return draft.repository.trim() === '';
		case 'DOCKER':
			return draft.repository.trim() === '';
		case 'ATTACHMENT':
			return draft.id === null;
	}
}

const optional = (text: string) => (text.trim() === '' ? undefined : text.trim());

/** The input the server takes for the editor's state: `null` clears the
 * field. */
export function inputOf(draft: Draft): unknown {
	if (isEmpty(draft)) return draft.kind === 'CHECKBOX' ? false : null;
	switch (draft.kind) {
		case 'TEXT':
			return draft.text;
		case 'CHOICE':
			return draft.key;
		case 'CHOICES':
			return draft.keys;
		case 'DATE':
			return draft.date;
		case 'USER':
			return 'user' in draft.who! ? draft.who.user : draft.who;
		case 'CHECKBOX':
			if (!draft.for) return true;
			return { for: 'user' in draft.for ? draft.for.user : draft.for };
		case 'REFERENCE':
			return { url: draft.url.trim(), label: optional(draft.label) };
		case 'GIT':
			return {
				repository: draft.repository.trim(),
				sha: optional(draft.sha),
				tag: optional(draft.tag)
			};
		case 'DOCKER':
			return {
				repository: draft.repository.trim(),
				digest: draft.digest.trim(),
				tag: optional(draft.tag)
			};
		case 'ATTACHMENT':
			return draft.id;
	}
}

/** The fields whose editors changed, as the input an edit takes. */
export function changes(
	fields: readonly FieldDef[],
	before: Record<string, Draft>,
	after: Record<string, Draft>
): Record<string, unknown> {
	const changed: Record<string, unknown> = {};
	for (const field of fields) {
		const from = before[field.key];
		const to = after[field.key];
		if (to && JSON.stringify(from) !== JSON.stringify(to)) {
			changed[field.key] = inputOf(to);
		}
	}
	return changed;
}

export function nameOf(someone: SomeoneShownFragment): string {
	return someone.__typename === 'Contact' ? someone.displayName : someone.name;
}

/** A value as text, as lists and boards show it. */
export function textOf(value: Value): string {
	switch (value.__typename) {
		case 'TextValue':
			return value.text;
		case 'ChoiceValue':
			return value.option.name;
		case 'ChoicesValue':
			return value.options.map((o) => o.name).join(', ');
		case 'DateValue':
			return value.date;
		case 'UserValue':
			return nameOf(value.user);
		case 'CheckboxValue': {
			const forName = nameOf(value.for);
			return forName === value.by.name
				? `Yes, ${value.by.name}`
				: `Yes, for ${forName} (checked by ${value.by.name})`;
		}
		case 'ReferenceValue':
			return value.label ?? value.url;
		case 'GitValue':
			return `${value.repository}${value.tag ? `@${value.tag}` : ''}${value.sha ? ` (${value.sha.slice(0, 12)})` : ''}`;
		case 'DockerValue':
			return `${value.repository}${value.tag ? `:${value.tag}` : ''}@${value.digest}`;
		case 'AttachmentValue':
			return value.attachment.name;
	}
}
