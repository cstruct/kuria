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

/** Runs `document` with `variables()` for as long as the component lives. */
export function query<Data, Variables extends AnyVariables>(
	document: TypedDocumentNode<Data, Variables>,
	variables: () => Variables
): Query<Data> {
	const client = getClient();
	const state = $state<{
		data: Data | undefined;
		error: CombinedError | undefined;
		fetching: boolean;
	}>({ data: undefined, error: undefined, fetching: true });
	$effect(() => {
		state.fetching = true;
		const subscription = client
			.query(document, variables(), { requestPolicy: 'cache-and-network' })
			.subscribe((result) => {
				state.data = result.data;
				state.error = result.error;
				state.fetching = result.stale;
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
