import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import { describe, expect, it } from 'vitest';
import * as syncProtocol from 'y-protocols/sync';
import * as Y from 'yjs';
import { colorOf, holdsAll, updateIn } from './live.svelte';

/** A sync message's body, as the server sends it after its type. */
function decoderOf(write: (encoder: encoding.Encoder) => void): decoding.Decoder {
	const encoder = encoding.createEncoder();
	write(encoder);
	return decoding.createDecoder(encoding.toUint8Array(encoder));
}

// r[verify app.live]
describe('a live draft in the app', () => {
	it('reads the update a sync message carries, leaving the message to read', () => {
		const doc = new Y.Doc();
		doc.getText('body').insert(0, 'Hello.');
		const update = Y.encodeStateAsUpdate(doc);
		const decoder = decoderOf((e) => syncProtocol.writeUpdate(e, update));
		expect(updateIn(decoder)).toEqual(update);
		expect(decoder.pos).toBe(0);
		// A step 1 carries none.
		expect(updateIn(decoderOf((e) => syncProtocol.writeSyncStep1(e, doc)))).toBeNull();
	});

	it('tells whether the server has every edit, deletions too', () => {
		const doc = new Y.Doc();
		const confirmed = new Y.Doc();
		doc.on('update', (update: Uint8Array, origin: unknown) => {
			if (origin === 'mine') return;
			Y.applyUpdate(confirmed, update);
		});
		doc.getText('body').insert(0, 'Hello.');
		expect(holdsAll(doc, confirmed)).toBe(true);
		const pending: Uint8Array[] = [];
		const record = (update: Uint8Array) => pending.push(update);
		doc.on('update', record);
		doc.transact(() => doc.getText('body').insert(5, ', world'), 'mine');
		expect(holdsAll(doc, confirmed)).toBe(false);
		Y.applyUpdate(confirmed, pending.pop()!);
		expect(holdsAll(doc, confirmed)).toBe(true);
		doc.transact(() => doc.getText('body').delete(0, 1), 'mine');
		expect(holdsAll(doc, confirmed)).toBe(false);
		Y.applyUpdate(confirmed, pending.pop()!);
		expect(holdsAll(doc, confirmed)).toBe(true);
	});

	it('gives each person one colour', () => {
		expect(colorOf('a')).toBe(colorOf('a'));
		expect(colorOf('a')).not.toBe(colorOf('b'));
	});
});
