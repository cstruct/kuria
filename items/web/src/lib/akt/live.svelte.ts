/**
 * An akt's live draft, open in the app (`app.live`): its Yjs document,
 * synced with the server over `/akter/{number}/live` (`live.sync`), who
 * else has it open (`live.presence`), and whether the person's edits are
 * kept yet.
 */
// r[impl app.live]
import * as decoding from 'lib0/decoding';
import { Awareness } from 'y-protocols/awareness';
import { messageAuth, messageSync, WebsocketProvider } from 'y-websocket';
import * as Y from 'yjs';

/** Someone else with the draft open, as the server names them. */
export interface Peer {
	client: number;
	id: string;
	name: string;
	color: string;
	/** The field their cursor is in, and where. */
	cursor: Cursor | null;
}

/** Where someone's cursor is: in a text field, as relative positions in
 * its `Y.Text`. */
export interface Cursor {
	field: string;
	anchor: unknown;
	head: unknown;
}

/** What the server tells a reader, on connecting and of each edit. */
export const READ_ONLY = 'you may read this live draft, not edit it';

export type Status = 'connecting' | 'connected' | 'disconnected';

/** A person's colour, the same wherever they are shown. */
export function colorOf(id: string): string {
	let hash = 0;
	for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
	return `hsl(${hash % 360} 70% 45%)`;
}

/** The server's sync message's update, if it is a step 2 or an update,
 * read without moving `decoder`. */
export function updateIn(decoder: decoding.Decoder): Uint8Array | null {
	const copy = decoding.createDecoder(decoder.arr);
	copy.pos = decoder.pos;
	const kind = decoding.readVarUint(copy);
	return kind === 1 || kind === 2 ? decoding.readVarUint8Array(copy) : null;
}

/** Whether `confirmed`, what the server sent, holds everything `doc` does. */
export function holdsAll(doc: Y.Doc, confirmed: Y.Doc): boolean {
	return Y.equalSnapshots(Y.snapshot(doc), Y.snapshot(confirmed));
}

export class LiveDraft {
	readonly doc = new Y.Doc();
	readonly awareness: Awareness;
	/** The origin of the person's own edits. */
	readonly origin = { local: true };
	status = $state<Status>('connecting');
	synced = $state(false);
	/** Whether some of the person's edits haven't reached the server yet. */
	pending = $state(false);
	/** Why the server last refused an edit, if it did. */
	refusal = $state<string | null>(null);
	/** Whether the server takes none of the person's edits: they may read
	 * the draft, not edit it. */
	readOnly = $state(false);
	peers = $state<Peer[]>([]);
	#provider: WebsocketProvider;
	/** The draft as the server sent it, to tell what it has. */
	#confirmed = new Y.Doc();
	#remote: (() => void)[] = [];

	constructor(number: string) {
		this.awareness = new Awareness(this.doc);
		// An awareness state's first clock is 0, which others ignore from a
		// client they haven't seen (y-protocols takes only a later clock), so
		// say something at once: the server stamps who this is.
		this.awareness.setLocalStateField('present', true);
		const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
		this.#provider = new WebsocketProvider(
			`${scheme}://${location.host}/akter`,
			`${encodeURIComponent(number)}/live`,
			this.doc,
			// Every edit goes through the server, which checks it.
			{ awareness: this.awareness, disableBc: true }
		);
		const provider = this.#provider;
		const sync = provider.messageHandlers[messageSync];
		provider.messageHandlers[messageSync] = (encoder, decoder, p, emitSynced, type) => {
			const update = updateIn(decoder);
			if (update) Y.applyUpdate(this.#confirmed, update);
			sync(encoder, decoder, p, emitSynced, type);
			this.#settle();
		};
		provider.messageHandlers[messageAuth] = (_encoder, decoder) => {
			// A refusal: its reason. What undoes the edit follows.
			if (decoding.readVarUint(decoder) !== 0) return;
			const reason = decoding.readVarString(decoder);
			if (reason === READ_ONLY) this.readOnly = true;
			else this.refusal = reason;
		};
		provider.on('status', ({ status }) => (this.status = status));
		provider.on('sync', (synced) => {
			this.synced = synced;
			this.#settle();
		});
		this.doc.on('update', (_update: Uint8Array, origin: unknown) => {
			if (origin === provider) {
				for (const listener of this.#remote) listener();
			} else {
				this.pending = true;
			}
		});
		this.awareness.on('change', () => this.#readPeers());
	}

	#settle() {
		this.pending = !holdsAll(this.doc, this.#confirmed);
	}

	#readPeers() {
		const peers: Peer[] = [];
		for (const [client, state] of this.awareness.getStates()) {
			if (client === this.doc.clientID) continue;
			const user = (state as { user?: { id?: string; name?: string } }).user;
			if (!user?.id) continue;
			peers.push({
				client,
				id: user.id,
				name: user.name ?? user.id,
				color: colorOf(user.id),
				cursor: (state as { cursor?: Cursor }).cursor ?? null
			});
		}
		this.peers = peers;
	}

	/** The `Y.Text` of the text field `key` (`live.fields`). */
	text(key: string): Y.Text {
		return this.doc.getText(key);
	}

	/** The map of every other field's value, as GraphQL answers it. */
	get fields(): Y.Map<unknown> {
		return this.doc.getMap('fields');
	}

	/** Sets the field `key` in the map to `value`, as GraphQL answers it;
	 * `null` takes it out. */
	setField(key: string, value: unknown): void {
		this.refusal = null;
		const clean = value === null || value === undefined ? null : JSON.parse(JSON.stringify(value));
		this.doc.transact(() => {
			if (clean === null) this.fields.delete(key);
			else this.fields.set(key, clean);
		}, this.origin);
	}

	/** Says where the person's cursor is, `null` for nowhere. */
	setCursor(cursor: Cursor | null): void {
		this.awareness.setLocalStateField('cursor', cursor);
	}

	/** Calls `listener` after each change from the server. */
	onRemote(listener: () => void): () => void {
		this.#remote.push(listener);
		return () => {
			this.#remote = this.#remote.filter((l) => l !== listener);
		};
	}

	/** Waits until the server has every edit the person made, for up to
	 * `wait` milliseconds, and says whether it has. */
	async settled(wait = 5000): Promise<boolean> {
		const until = Date.now() + wait;
		while (this.pending || !this.synced) {
			if (Date.now() > until) return false;
			await new Promise((done) => setTimeout(done, 100));
		}
		return true;
	}

	destroy(): void {
		this.#provider.destroy();
		this.awareness.destroy();
		this.doc.destroy();
		this.#confirmed.destroy();
	}
}
