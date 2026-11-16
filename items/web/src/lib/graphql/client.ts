import { Client, fetchExchange, mapExchange, subscriptionExchange } from '@urql/core';
import { cacheExchange, type Cache, type CacheExchangeOpts } from '@urql/exchange-graphcache';
import { createClient as createWsClient } from 'graphql-ws';
import introspection from './generated/introspection.json';
import { keysOf } from './keys';
import { signIn, signedOut } from '../auth';

/** The schema, as Graphcache wants it, with its object types' keys. */
const schema = introspection as unknown as NonNullable<CacheExchangeOpts['schema']>;
const types = (introspection as { __schema: { types: { kind: string; name: string }[] } }).__schema
	.types;
const keys = keysOf(types, ['Query', 'Mutation', 'ChangesSubscription']);

/**
 * Marks what a change event makes stale: the akt itself, and every list,
 * board and membership that may hold it, so the views showing them refetch.
 */
// r[impl app.live]
export function invalidateChange(cache: Cache, akt: string): void {
	cache.invalidate({ __typename: 'Akt', id: akt });
	for (const field of cache.inspectFields('Query')) {
		if (field.fieldName === 'akter') {
			cache.invalidate('Query', field.fieldName, field.arguments ?? undefined);
		}
		if (field.fieldName === 'akt') {
			const entity = cache.resolve('Query', field.fieldName, field.arguments ?? undefined);
			if (typeof entity === 'string') {
				for (const name of ['board', 'members', 'containers']) {
					for (const inner of cache.inspectFields(entity)) {
						if (inner.fieldName === name) {
							cache.invalidate(entity, inner.fieldName, inner.arguments ?? undefined);
						}
					}
				}
			}
		}
	}
}

/**
 * Marks every akt type's count stale, so the sidebar and home count again
 * after an akt is archived or unarchived; they count live akter only.
 */
// r[impl app.archive]
export function invalidateCounts(cache: Cache): void {
	for (const field of cache.inspectFields('Query')) {
		if (field.fieldName !== 'aktTypes') continue;
		const types = cache.resolve('Query', field.fieldName, field.arguments ?? undefined);
		for (const type of Array.isArray(types) ? types : []) {
			if (typeof type !== 'string') continue;
			for (const inner of cache.inspectFields(type)) {
				if (inner.fieldName === 'akterCount') {
					cache.invalidate(type, inner.fieldName, inner.arguments ?? undefined);
				}
			}
		}
	}
}

/** The app's GraphQL client: queries and mutations over HTTP, the
 * `changes` subscription over a WebSocket, both on this origin. */
export function createClient(): Client {
	const ws = createWsClient({
		url: `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/graphql/ws`,
		lazy: true,
		retryAttempts: Infinity
	});
	return new Client({
		url: '/graphql',
		// The server takes GraphQL only as a JSON POST.
		preferGetMethod: false,
		fetchOptions: { credentials: 'same-origin' },
		exchanges: [
			mapExchange({
				onError(error) {
					if (signedOut(error.response?.status)) signIn();
				}
			}),
			cacheExchange({
				schema,
				keys,
				updates: {
					Mutation: {
						archiveAkt(result, _args, cache) {
							const akt = (result as { archiveAkt?: { id: string } }).archiveAkt;
							if (akt) invalidateChange(cache, akt.id);
							invalidateCounts(cache);
						},
						unarchiveAkt(result, _args, cache) {
							const akt = (result as { unarchiveAkt?: { id: string } }).unarchiveAkt;
							if (akt) invalidateChange(cache, akt.id);
							invalidateCounts(cache);
						}
					},
					ChangesSubscription: {
						changes(result, _args, cache) {
							const event = (result as { changes?: { akt: string } }).changes;
							if (event) invalidateChange(cache, event.akt);
						}
					}
				}
			}),
			fetchExchange,
			subscriptionExchange({
				forwardSubscription(request) {
					const input = { ...request, query: request.query ?? '' };
					return {
						subscribe(sink) {
							const unsubscribe = ws.subscribe(input, sink);
							return { unsubscribe };
						}
					};
				}
			})
		]
	});
}
