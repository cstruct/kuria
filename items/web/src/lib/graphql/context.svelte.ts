import { getContext, setContext } from 'svelte';
import type { AnyVariables, Client, CombinedError, TypedDocumentNode } from '@urql/core';

const KEY = Symbol('urql');

export function setClient(client: Client): void {
	setContext(KEY, client);
}

export function getClient(): Client {
	return getContext<Client>(KEY);
}

/** A query's latest result, kept current: refetched when its variables
 * change, and when the cache marks what it read as stale. */
export interface Query<Data> {
	readonly data: Data | undefined;
	readonly error: CombinedError | undefined;
	readonly fetching: boolean;
}

/** Runs `document` with `variables()` for as long as the component lives,
 * asking again only when the variables' values change: a variable built
 * from what the cache answered is a new object whenever the cache updates,
 * and asking again for it would ask forever. */
export function query<Data, Variables extends AnyVariables>(
	document: TypedDocumentNode<Data, Variables>,
	variables: () => Variables,
	/** Typenames besides those it holds whose changes make it stale, such
	 * as `Akt` for counts of akter. */
	additionalTypenames: string[] = []
): Query<Data> {
	const client = getClient();
	const state = $state<{
		data: Data | undefined;
		error: CombinedError | undefined;
		fetching: boolean;
	}>({ data: undefined, error: undefined, fetching: true });
	const asked = $derived(JSON.stringify(variables()));
	$effect(() => {
		state.fetching = true;
		const subscription = client
			.query(document, JSON.parse(asked) as Variables, {
				requestPolicy: 'cache-and-network',
				additionalTypenames
			})
			.subscribe((result) => {
				// While what the cache held is refetched, after a change marked it
				// stale, keep showing it rather than nothing.
				if (result.data !== undefined || result.error) state.data = result.data;
				state.error = result.error;
				state.fetching = result.stale || result.data === undefined;
			});
		return () => subscription.unsubscribe();
	});
	return state;
}

/** Runs a mutation; its result, or the error it was refused with. */
export async function mutate<Data, Variables extends AnyVariables>(
	client: Client,
	document: TypedDocumentNode<Data, Variables>,
	variables: Variables
): Promise<{ data?: Data; error?: CombinedError }> {
	const result = await client.mutation(document, variables).toPromise();
	return { data: result.data, error: result.error };
}
