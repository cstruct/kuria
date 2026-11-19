<script lang="ts">
	import Pencil from '@lucide/svelte/icons/pencil';
	import { orList } from '#lib/ui/text.ts';
	import TypeMark from '#lib/akt/TypeMark.svelte';
	import { resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import { SvelteSet } from 'svelte/reactivity';
	import { page } from '$app/state';
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
	import Copy from '@lucide/svelte/icons/copy';
	import Link2 from '@lucide/svelte/icons/link-2';
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { before, crumbsOf } from '#lib/shell/trail.ts';
	import { setFollowing } from '#lib/akt/following.ts';
	import FieldRow from '#lib/akt/FieldRow.svelte';
	import HistoryList from '#lib/akt/HistoryList.svelte';
	import CopyDialog from '#lib/akt/CopyDialog.svelte';
	import Members from '#lib/akt/Members.svelte';
	import OutcomeBadge from '#lib/akt/OutcomeBadge.svelte';
	import Sections from '#lib/akt/Sections.svelte';
	import { numbersIn, readSections } from '#lib/akt/sections.ts';
	import { judgedOf, judgingOpen, verdictInput, type Report } from '#lib/akt/verdicts.ts';
	import LinksCard, { type ShownLink } from '#lib/akt/LinksCard.svelte';
	import TransitionDialog from '#lib/akt/TransitionDialog.svelte';
	import {
		availableTransitions,
		outcomeRefusal,
		containedTypes,
		linkKinds,
		draftedFields,
		releasedFile,
		requiredFields,
		givenIn
	} from '#lib/akt/document.ts';
	import ReleasedPdf from '#lib/akt/ReleasedPdf.svelte';
	import { upload } from '#lib/akt/upload.ts';
	import { oneField, settle } from '#lib/akt/editing.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import {
		archivedCount,
		archivedFilterOf,
		archivedSummary,
		showsArchived,
		withArchived
	} from '#lib/akt/archive.ts';
	import { actionsOf, confirmationOf, headingOf } from '#lib/akt/header.ts';
	import { peopleOf, type Person } from '#lib/akt/people.ts';
	import { pickable } from '#lib/akt/pickers.ts';
	import { undoOf } from '#lib/akt/members.ts';
	import { readiness } from '#lib/akt/outcomes.ts';
	import { parentOf } from '#lib/akt/pulling.ts';
	import PullDialog from '#lib/akt/PullDialog.svelte';
	import {
		creatableIn,
		insideQuery,
		parentBoards,
		quickAddable,
		titleFieldOf
	} from '#lib/akt/creating.ts';
	import { changes, draftOf, inputOf, type Draft, type FieldDef } from '#lib/akt/values.ts';
	import { LiveDraft } from '#lib/akt/live.svelte.ts';
	import LivePresence from '#lib/akt/LivePresence.svelte';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import type { AktPageQuery } from '#lib/graphql/generated/graphql.ts';
	import {
		AddLink,
		AddToContainer,
		ArchiveAkt,
		UnarchiveAkt,
		RemoveLink,
		AktPage,
		Batch,
		Contacts,
		CopyPlaces,
		CopyAkt,
		CreateAkt,
		Users,
		DraftField,
		EditAkt,
		MoveAkt,
		PerformTransition,
		ReleaseVersion,
		ReleaseVersions,
		AktRevision,
		RemoveFromContainer,
		TransferMember
	} from '#lib/graphql/operations.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import VersionsCard from '#lib/akt/VersionsCard.svelte';
	import DiffDialog from '#lib/akt/DiffDialog.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { toast } from '#lib/ui/toast.svelte.ts';

	type Akt = NonNullable<AktPageQuery['akt']>;
	type Transition = Akt['type']['transitions'][number];
	type Member = Akt['board']['columns'][number]['akter'][number];
	type MemberTransition = Member['type']['transitions'][number];

	const client = getClient();
	const number = $derived(page.params.number ?? '');
	// r[impl app.versions]: `?version=` reads the akt at one of its versions.
	const atVersion = $derived(Number(page.url.searchParams.get('version')) || null);
	// r[impl app.archive]: a board shows live members, unless the container
	// is archived, or with Show archived, all of them.
	const showArchived = $derived(showsArchived(page.url.searchParams));
	const boardArchived = $derived(archivedFilterOf(page.url.searchParams));
	const view = query(AktPage, () => ({ number, version: atVersion, boardArchived }));
	const contacts = query(Contacts, () => ({}));
	const users = query(Users, () => ({}));

	// While a refetch is under way the query has no data: the akt shown
	// stays until it answers, so nothing on the page, such as an editor
	// someone is typing in, is taken down and put back.
	let shown: { number: string; version: number | null; akt: Akt } | undefined;
	const akt = $derived.by(() => {
		const read = view.data?.akt ?? undefined;
		if (read) {
			shown = { number, version: atVersion, akt: read };
			return read;
		}
		return shown && shown.number === number && shown.version === atVersion ? shown.akt : undefined;
	});
	// r[impl app.trail]: the type, then the akter followed here, and this
	// one, each with its title.
	const trail = $derived(page.state.trail ?? []);
	// r[impl app.trail]: where it is, its first container's own first.
	const places = $derived(
		akt?.containers[0]
			? [...akt.containers[0].containers.slice(0, 1), akt.containers[0]].map((c) => ({
					number: c.number,
					title: c.title ?? null,
					href: resolve('/akt/[number]', { number: c.number })
				}))
			: []
	);
	setCrumbs(() =>
		akt
			? crumbsOf(
					trail,
					{
						number: akt.number,
						title: akt.title ?? null,
						type: { key: akt.type.key, name: akt.type.name }
					},
					(key) => resolve('/types/[key]', { key }),
					places
				)
			: [{ label: number }]
	);
	// Following a reference in a shown field goes on from here.
	setFollowing(() =>
		akt
			? [
					...before(trail, akt.number),
					{
						number: akt.number,
						title: akt.title ?? null,
						type: { key: akt.type.key, name: akt.type.name },
						href: `${page.url.pathname}${page.url.search}`,
						// The trail's start, carried on from its first page.
						...(before(trail, akt.number).length === 0 ? { places } : {})
					}
				]
			: null
	);
	const fieldNames = $derived(
		Object.fromEntries((akt?.type.fields ?? []).map((f) => [f.key, f.name]))
	);
	const transitionNames = $derived(
		Object.fromEntries((akt?.type.transitions ?? []).map((t) => [t.key, t.name]))
	);
	const people = $derived<Person[]>(
		peopleOf(
			view.data?.viewer.subject,
			users.data?.users,
			contacts.data?.contacts,
			view.data?.viewer.user
		)
	);
	const typeNames = $derived(
		Object.fromEntries((view.data?.aktTypes ?? []).map((t) => [t.key, t.name]))
	);
	const contained = $derived(akt ? containedTypes(akt.type.document) : []);
	/** The parent board it pulls members from, if it links to one. */
	const parent = $derived(akt ? parentOf(akt.type.document, akt.links) : null);
	let pullOpen = $state(false);
	/** The types the viewer may create, by key. */
	const creatableKeys = $derived(
		new Set((view.data?.aktTypes ?? []).filter((t) => t.viewerMay.create).map((t) => t.key))
	);
	/** The contained types, as people name them: "task or epic". */
	const containedWords = $derived(
		orList(contained.map((key) => (typeNames[key] ?? key).toLowerCase()))
	);
	/** What moving it elsewhere is called: "Move to another release". */
	const moveWords = $derived(
		`Move to another ${akt?.containers[0]?.type.name.toLowerCase() ?? 'place'}`
	);
	/** The new-akt page that creates an akt of the type `key` in this
	 * container and in its parent boards that take it (`app.board-create`). */
	// r[impl app.board-create]
	const parentsFor = (key: string) =>
		akt ? parentBoards(akt.type.document, akt.links, key, view.data?.aktTypes ?? []) : [];
	// r[impl app.create]: made from here, it comes back here.
	const newMemberHref = (key: string) =>
		akt
			? `${resolve('/types/[key]/new', { key })}?${insideQuery(akt.number, parentsFor(key))}&return=1`
			: resolve('/types/[key]/new', { key });

	/** Creates an akt of the type `key` titled `title` in this container
	 * and its parent boards that take it, staying on the board
	 * (`app.quick-add`). */
	// r[impl app.quick-add]
	async function quickAdd(key: string, title: string): Promise<boolean> {
		const type = view.data?.aktTypes.find((t) => t.key === key);
		const field = type ? titleFieldOf(type.document) : null;
		if (!akt || !field) return false;
		const into = [akt.number, ...parentsFor(key).filter((p) => p !== akt.number)];
		const created = await run(() =>
			mutate(client, CreateAkt, {
				input: { type: key, fields: { [field]: title }, in: into, reason: null }
			})
		);
		if (created) toast(`Added ${title}`, 'success');
		return created;
	}
	// r[impl app.permitted]: only the types the viewer may create.
	const creatable = (column: string) =>
		akt && editable
			? creatableIn(
					akt.type.document,
					column,
					(view.data?.aktTypes ?? []).filter((t) => t.viewerMay.create)
				).map((t) => ({
					key: t.key,
					name: t.name,
					href: newMemberHref(t.key),
					quick: quickAddable(t.document)
				}))
			: [];
	const drafted = $derived(akt ? draftedFields(akt.type.document) : []);
	// A version never changes: read at one, nothing is offered to change.
	/** The key of the first column of the akt's board, if it has one. */
	const firstColumn = $derived(
		(akt?.type.document as { board?: { columns?: { key?: string }[] } } | null)?.board?.columns?.[0]
			?.key
	);
	// r[impl app.header]: the action offered first, by the presentation,
	// else creating a member; never a transition by default.
	const actions = $derived(
		actionsOf(
			// r[impl app.permitted]: transitions only to one who may perform them.
			akt && !atVersion && !akt.archived && akt.type.viewerMay.transition
				? availableTransitions(akt.type.transitions, akt.state)
				: [],
			{
				document: akt?.type.document,
				state: akt?.state.key,
				copyable: Boolean(akt?.type.viewerMay.create && !atVersion),
				creatable: firstColumn ? creatable(firstColumn) : []
			}
		)
	);
	// r[impl app.verdicts]: verdicts only while their run takes them: its
	// members' while it is in such a state, its own while its container is.
	const judgingHere = $derived(akt ? judgingOpen(akt.type, akt.state.key) : true);
	const judgingOwn = $derived.by(() => {
		const container = akt?.containers[0];
		if (!container) return true;
		const document = view.data?.aktTypes.find((t) => t.key === container.type.key)?.document as {
			transitions?: { from: string[]; outcomes?: string[] }[];
		} | null;
		return judgingOpen({ transitions: document?.transitions ?? [] }, container.state.key);
	});
	/** The field its title is, and whether its details would show nothing
	 * else: then the heading is edited, and the details aren't shown. */
	const titleKey = $derived(akt ? titleFieldOf(akt.type.document) : null);
	const titleOnly = $derived(
		Boolean(
			akt &&
			titleKey &&
			akt.type.fields.every(
				(f) =>
					f.key === titleKey ||
					(!givenIn(akt.type.document, f.key, akt.state.key) &&
						!akt.fields.some((v) => 'field' in v && v.field === f.key))
			)
		)
	);
	/** Of its members with outcomes, how many passed (`app.header`). */
	const ready = $derived(akt ? readiness(akt.board.columns.flatMap((c) => c.akter)) : null);
	const primaryTransition = $derived(
		actions.primary?.kind === 'transition' ? actions.primary.transition : undefined
	);
	// r[impl app.live]: an akt with a live draft is its live draft, edited
	// together with everyone else who has it open.
	// Kept while a refetch has no akt yet, so that refreshing the page's data
	// doesn't drop the connection and the editor bound to it.
	let liveFor = $state<string | null>(null);
	$effect(() => {
		if (akt) liveFor = akt.type.liveDrafts && !atVersion && !akt.archived ? akt.number : null;
	});
	let live = $state<LiveDraft | null>(null);
	$effect(() => {
		if (!liveFor) return;
		const opened = new LiveDraft(liveFor);
		live = opened;
		return () => {
			opened.destroy();
			if (live === opened) live = null;
		};
	});
	/** Whether the field is a text of the live draft, edited as it is typed. */
	const isLiveText = (field: FieldDef) =>
		field.kind === 'MARKDOWN' || (field.kind === 'TEXT' && field.multiline);
	const editable = $derived(
		// A live draft is edited once it is in hand, by those who may.
		Boolean(
			akt &&
			// r[impl app.permitted]: edits only to one who may make them.
			akt.type.viewerMay.edit &&
			!akt.state.final &&
			!atVersion &&
			// r[impl app.archive]: an archived akt offers no edit.
			!akt.archived &&
			!(live && (live.readOnly || !live.synced))
		)
	);
	const versioned = $derived(
		Boolean(akt && (akt.type.document as { versions?: unknown } | null)?.versions)
	);
	// r[impl app.pdf]: each markdown field as a PDF, of what is being read,
	// to download.
	const markdownFields = $derived((akt?.type.fields ?? []).filter((f) => f.kind === 'MARKDOWN'));
	// r[impl app.external]: an akt released as files has the PDF of the
	// version read, else of its latest; one written here, the PDF of a
	// version released with one, and its body's sheets otherwise.
	const fileField = $derived(akt ? releasedFile(akt.type.document) : null);
	const writtenHere = $derived(markdownFields.length > 0);
	const readsFile = $derived(
		Boolean(
			fileField && atVersion && akt?.fields.some((v) => 'field' in v && v.field === fileField)
		)
	);
	const showsFile = $derived(Boolean(fileField) && (!writtenHere || readsFile));
	const shownVersion = $derived(
		writtenHere ? atVersion : (atVersion ?? akt?.versions[0]?.number ?? null)
	);
	const pdfs = $derived(
		showsFile
			? shownVersion === null
				? []
				: [
						{
							key: fileField,
							label: 'Download PDF',
							href: `/akter/${encodeURIComponent(number)}/pdf?version=${shownVersion}&download`
						}
					]
			: markdownFields.map((f, _, all) => ({
					key: f.key,
					label: all.length > 1 ? `${f.name} as PDF` : 'Download PDF',
					href: `/akter/${encodeURIComponent(number)}/pdf?field=${f.key}${atVersion ? `&version=${atVersion}` : ''}&download`
				}))
	);
	let comparing = $state<{ from: number; to: number | null } | null>(null);
	let compareOpen = $state(false);

	async function releaseVersion(
		label: string,
		together: string[] = [],
		file: File | null = null,
		description = ''
	) {
		if (!akt) return false;
		// r[impl app.external]: the PDF uploaded, then released with.
		let uploaded: string | null = null;
		if (file) {
			try {
				uploaded = (await upload(file)).id;
			} catch (e) {
				toast(e instanceof Error ? e.message : String(e), 'danger');
				return false;
			}
		}
		// r[impl app.live]: the live draft as it is, with every edit made.
		if (live && !(await live.settled())) {
			toast('Your last edits haven’t reached the server yet: try again', 'danger');
			return false;
		}
		const named = label.trim() || null;
		const said = description.trim() || null;
		const input = {
			number: akt.number,
			revision: akt.revision,
			label: named,
			description: said,
			file: uploaded
		};
		if (together.length === 0) {
			const released = await run(() => mutate(client, ReleaseVersion, { input }));
			if (released) toast(`${headingOf(akt)} released`, 'success');
			return released;
		}
		// r[impl app.versions]: released together, each at the revision read now.
		const others: {
			number: string;
			revision: number;
			label: string | null;
			description: string | null;
		}[] = [];
		for (const number of together) {
			const read = await client
				.query(AktRevision, { number }, { requestPolicy: 'network-only' })
				.toPromise();
			const other = read.data?.akt;
			if (!other) {
				toast(`${number} can’t be read`, 'danger');
				return false;
			}
			others.push({ number, revision: other.revision, label: named, description: said });
		}
		const released = await run(() =>
			mutate(client, ReleaseVersions, { input: { akter: [input, ...others] } })
		);
		if (released) toast(`${headingOf(akt)} released, with ${together.length} more`, 'success');
		return released;
	}
	const me = $derived(view.data?.viewer.subject);

	// r[impl app.sections]: the members in the akt's sections, and those
	// no section names after them, as the board and lists show them.
	const sectionItems = $derived.by(() => {
		const def = akt?.type.fields.find((f) => f.kind === 'SECTIONS');
		const value = akt?.fields.find((v) => v.__typename === 'SectionsValue' && v.field === def?.key);
		return value?.__typename === 'SectionsValue' ? readSections(value.value) : [];
	});
	const allMembers = $derived(akt ? akt.board.columns.flatMap((c) => c.akter) : []);
	const memberNames = $derived(allMembers.map((m) => ({ number: m.number, title: m.title })));
	const placed = $derived(new Set(numbersIn(sectionItems)));
	const unplacedColumns = $derived(
		(akt?.board.columns ?? []).map((c) => ({
			...c,
			akter: c.akter.filter((m) => !placed.has(m.number))
		}))
	);
	const ownVerdicts = $derived(akt ? judgedOf(akt, me) : null);
	let copyOpen = $state(false);
	/** Where a copy may go: the open akter of the types that hold this
	 * one's, the most recently changed first (`app.copy`). */
	let copyPlaces = $state<{ number: string; title: string }[]>([]);
	$effect(() => {
		if (!copyOpen || !akt) return;
		const key = akt.type.key;
		// r[impl app.permitted]: only into what the viewer may add to.
		const holders = (view.data?.aktTypes ?? []).filter(
			(t) => t.viewerMay.edit && containedTypes(t.document).includes(key)
		);
		void Promise.all(
			holders.map((t) => client.query(CopyPlaces, { type: t.key }).toPromise())
		).then((answers) => {
			copyPlaces = answers
				.flatMap((a) => a.data?.akter.nodes ?? [])
				.filter((n) => !n.state.final)
				.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
				.map((n) => ({ number: n.number, title: n.title ?? n.number }));
		});
	});
	// The collapsible sections opened, kept while the akt reloads.
	const opened = new SvelteSet<string>();

	// What the editors hold, and what they held when the akt was loaded.
	// Kept across refetches, so a refusal or a change made elsewhere never
	// loses what the person entered.
	let base = $state<Record<string, Draft>>({});
	let drafts = $state<Record<string, Draft>>({});
	let loadedFor = $state('');
	let editingField = $state<string | null>(null);
	let pending = $state<Transition | null>(null);
	let transitionOpen = $state(false);
	let reason = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);
	let transferOpen = $state(false);
	let attachOpen = $state(false);
	let transferFrom = $state('');
	let addOpen = $state(false);
	// A member dropped on another column, and the transitions that lead there.
	let moving = $state<{ member: Member; transitions: MemberTransition[]; column: string } | null>(
		null
	);
	let movingOpen = $state(false);
	// The transition `?transition=` asked for, opened once per akt.
	let askedFor = $state('');
	// r[impl app.references]: a reference's heading, scrolled to once the
	// akt it is in has loaded, since it isn't there when the page opens.
	let scrolledTo = $state('');
	$effect(() => {
		const hash = page.url.hash.slice(1);
		if (!akt || !hash || scrolledTo === `${number}#${hash}`) return;
		scrolledTo = `${number}#${hash}`;
		requestAnimationFrame(() => document.getElementById(hash)?.scrollIntoView());
	});

	function snapshot(of: Akt): Record<string, Draft> {
		return Object.fromEntries(
			of.type.fields.map((field) => [
				field.key,
				draftOf(
					field,
					of.fields.find((v) => 'field' in v && v.field === field.key)
				)
			])
		);
	}

	$effect(() => {
		if (akt && loadedFor !== akt.number) {
			base = snapshot(akt);
			drafts = snapshot(akt);
			loadedFor = akt.number;
			opened.clear();
			editingField = null;
		}
	});

	// r[impl app.board-drop]: a transition chosen on a container's board
	// that needs fields opens here, asking for them.
	$effect(() => {
		const key = page.url.searchParams.get('transition');
		if (!akt || !key || askedFor === akt.number || akt.archived) return;
		askedFor = akt.number;
		const transition = availableTransitions(akt.type.transitions, akt.state).find(
			(t) => t.key === key
		);
		if (transition) begin(transition);
	});

	async function refetch(): Promise<Akt | undefined> {
		const result = await client
			.query(AktPage, { number, boardArchived }, { requestPolicy: 'network-only' })
			.toPromise();
		return result.data?.akt ?? undefined;
	}

	/** Runs a change; on success, reloads the editors from the result. A
	 * refusal shows where it belongs: in the open editor or dialog, else as a
	 * toast. */
	async function run(change: () => Promise<{ error?: import('@urql/core').CombinedError }>) {
		busy = true;
		refusal = null;
		const { error } = await change();
		const fresh = await refetch();
		busy = false;
		const settled = settle(error, fresh ? snapshot(fresh) : undefined, fieldNames);
		refusal = settled.refusal;
		if (settled.base) base = settled.base;
		if (settled.drafts) drafts = settled.drafts;
		if (refusal && !editingField && !transitionOpen) toast(refusal.message, 'danger');
		return !error;
	}

	async function save(key: string) {
		if (!akt) return;
		// r[impl app.live]: a field of a live draft is set in it, but for a
		// checkbox, which the server checks for whoever checks it.
		const kind = akt.type.fields.find((f) => f.key === key)?.kind;
		if (live && kind !== 'CHECKBOX' && drafts[key]) {
			if (JSON.stringify(base[key]) !== JSON.stringify(drafts[key])) {
				live.setField(key, inputOf(drafts[key]));
				base[key] = structuredClone($state.snapshot(drafts[key]));
			}
			editingField = null;
			if (await live.settled(2000)) await refetch();
			return;
		}
		const fields = oneField(key, base[key], drafts[key]);
		if (Object.keys(fields).length === 0) {
			editingField = null;
			return;
		}
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, EditAkt, { input: { number, revision, fields, reason: reason || null } })
			)
		) {
			editingField = null;
			reason = '';
		}
	}

	function cancelEdit(key: string) {
		drafts[key] = structuredClone($state.snapshot(base[key]));
		editingField = null;
		refusal = null;
		reason = '';
	}

	function required(transition: Transition): FieldDef[] {
		return requiredFields(akt?.type.fields ?? [], transition);
	}

	function begin(transition: Transition) {
		refusal = null;
		pending = transition;
		transitionOpen = true;
	}

	async function perform() {
		if (!akt || !pending) return;
		const transition = pending;
		const fields = changes(required(transition), base, drafts);
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, PerformTransition, {
					input: { number, revision, transition: transition.key, fields, reason: reason || null }
				})
			)
		) {
			transitionOpen = false;
			pending = null;
			reason = '';
			// r[impl app.toasts]: what changed, and where it went.
			const to = akt.type.states.find((s) => s.key === transition.to);
			toast(`${headingOf(akt)} moved to ${to?.name ?? transition.name}`, 'success');
		}
	}

	// r[impl app.board-drop]: one move that needs nothing is made at once;
	// otherwise the choice of transitions, or their fields, is asked for.
	function into(member: Member, transitions: MemberTransition[], column: string) {
		moving = { member, transitions, column };
		const only = transitions.length === 1 ? transitions[0] : null;
		if (only && only.requires.length === 0) {
			void moveInto(only);
			return;
		}
		movingOpen = true;
	}

	/** Moves a member back the way it came (`app.board-drop`). */
	async function undoMove(number: string, title: string, transition: MemberTransition) {
		const fresh = await refetch();
		const member = fresh?.board.columns.flatMap((c) => c.akter).find((m) => m.number === number);
		if (!member) return;
		const input = { number, revision: member.revision, transition: transition.key };
		if (await run(() => mutate(client, PerformTransition, { input }))) {
			toast(`${title} moved back`, 'success');
		}
	}

	/** Performs a transition on the member being moved, or, when it needs
	 * fields, opens it on the member's page. */
	async function moveInto(transition: MemberTransition) {
		if (!moving) return;
		const { member } = moving;
		if (transition.requires.length > 0) {
			const query = `?transition=${encodeURIComponent(transition.key)}`;
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved, with a query
			await goto(`${resolve('/akt/[number]', { number: member.number })}${query}`);
			return;
		}
		const input = { number: member.number, revision: member.revision, transition: transition.key };
		if (await run(() => mutate(client, PerformTransition, { input }))) {
			movingOpen = false;
			const title = member.title ?? member.number;
			const back = undoOf(member.type.transitions, member.state.key, transition.to);
			toast(
				`${title} moved to ${moving.column}`,
				'success',
				5000,
				back ? { label: 'Undo', run: () => void undoMove(member.number, title, back) } : undefined
			);
		}
	}

	/** Reports the viewer's verdict on `of`, this akt or a member, or
	 * deletes it. */
	// r[impl app.verdicts]
	async function giveVerdict(
		of: {
			number: string;
			revision: number;
			fields: Akt['fields'];
			type: Akt['type'] | Member['type'];
		},
		verdict: Report | null
	) {
		const judged = judgedOf(of, me);
		if (!judged) return;
		const input = {
			number: of.number,
			revision: of.revision,
			fields: { [judged.field]: verdictInput(verdict) }
		};
		await run(() => mutate(client, EditAkt, { input }));
	}

	/** Copies the akt, keeping `only` of its members, and opens the copy. */
	// r[impl app.copy]
	async function copy(only: string[] | null, into: string | null) {
		busy = true;
		refusal = null;
		const { data, error } = await mutate(client, CopyAkt, {
			input: { number, only, ...(into ? { in: [into] } : {}) }
		});
		if (error || !data) {
			busy = false;
			refusal = refusalOf(error!, fieldNames);
			return;
		}
		// r[impl app.copy]: a copy made for a container is named after it.
		const place = copyPlaces.find((p) => p.number === into);
		const titled = akt ? titleFieldOf(akt.type.document) : null;
		if (place && titled && akt?.title) {
			await mutate(client, EditAkt, {
				input: {
					number: data.copyAkt.number,
					revision: data.copyAkt.revision,
					fields: { [titled]: `${akt.title}: ${place.title}` },
					reason: `Run for ${place.title}`
				}
			});
		}
		busy = false;
		copyOpen = false;
		toast(`Copied to ${data.copyAkt.title ?? data.copyAkt.number}`, 'success');
		await goto(resolve('/akt/[number]', { number: data.copyAkt.number }));
	}

	async function draft(field: string) {
		if (!akt) return;
		const revision = akt.revision;
		if (await run(() => mutate(client, DraftField, { input: { number, revision, field } }))) {
			toast(`Drafted ${fieldNames[field] ?? field}`, 'success');
		}
	}

	async function addLink(kind: string, target: string) {
		await run(() => mutate(client, AddLink, { input: { source: number, kind, target } }));
	}

	async function removeLink(link: ShownLink) {
		const [source, target] = link.outgoing ? [number, link.akt.number] : [link.akt.number, number];
		await run(() => mutate(client, RemoveLink, { input: { source, kind: link.kind, target } }));
	}

	async function join(container: string) {
		if (await run(() => mutate(client, AddToContainer, { input: { container, member: number } }))) {
			attachOpen = false;
		}
	}

	async function leave(container: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container, member: number } }));
	}

	async function transfer(to: string) {
		if (!transferFrom) return;
		const from = transferFrom;
		if (await run(() => mutate(client, TransferMember, { input: { member: number, from, to } }))) {
			transferOpen = false;
			toast(`Moved from ${from} to ${to}`, 'success');
		}
	}

	/** Adds members of the parent board here, all or none (`app.pull`). */
	// r[impl app.pull]
	async function pull(numbers: string[]) {
		if (!parent || numbers.length === 0) return;
		const from = parent.title;
		const operations = numbers.map((member) => ({
			addToContainer: { container: number, member }
		}));
		if (await run(() => mutate(client, Batch, { input: { operations } }))) {
			pullOpen = false;
			toast(`Added ${numbers.length} from ${from}`, 'success');
		}
	}

	async function addMember(member: string) {
		addOpen = false;
		await run(() => mutate(client, AddToContainer, { input: { container: number, member } }));
	}

	async function removeMember(member: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container: number, member } }));
	}

	async function move(member: string, place: { before?: string; after?: string }) {
		await run(() => mutate(client, MoveAkt, { input: { number: member, ...place } }));
	}

	// r[impl app.archive]: archiving and unarchiving, with a reason if given.
	let archiving = $state<'archive' | 'unarchive' | null>(null);
	let archiveOpen = $state(false);
	let archiveReason = $state('');
	let archiveRefusal = $state<Refusal | null>(null);
	const archived = $derived(akt?.archived ? archivedSummary(akt.archived) : null);

	function beginArchiving(kind: 'archive' | 'unarchive') {
		archiving = kind;
		archiveReason = '';
		archiveRefusal = null;
		archiveOpen = true;
	}

	async function setArchived() {
		if (!akt || !archiving) return;
		const kind = archiving;
		const target = akt.number;
		const heading = headingOf(akt);
		busy = true;
		archiveRefusal = null;
		const input = { number: target, reason: archiveReason.trim() || null };
		const { error } =
			kind === 'archive'
				? await mutate(client, ArchiveAkt, { input })
				: await mutate(client, UnarchiveAkt, { input });
		if (error) {
			busy = false;
			archiveRefusal = refusalOf(error);
			return;
		}
		await refetch();
		busy = false;
		archiveOpen = false;
		toast(`${heading} ${kind === 'archive' ? 'archived' : 'unarchived'}`, 'success');
	}

	function toggleArchivedMembers(on: boolean) {
		const search = withArchived(page.url.searchParams, on).toString();
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved, with a query
		goto(`${resolve('/akt/[number]', { number })}${search ? `?${search}` : ''}`, {
			replace: true,
			reset: false
		});
	}
</script>

<svelte:head>
	<title>{akt ? `${headingOf(akt)} · ${akt.number}` : number} · Kuria</title>
</svelte:head>

<main class="akt">
	{#if view.error && !akt}
		<p role="alert">{refusalOf(view.error).message}</p>
	{:else if akt}
		<!-- r[impl app.header] -->
		<header class="head">
			<div class="identity">
				<p class="eyebrow">
					<a href={resolve('/types/[key]', { key: akt.type.key })}
						><TypeMark type={akt.type} named /></a
					>
					<span data-testid="number"><Mono value={akt.number} max={40} copy /></span>
				</p>
				<!-- r[impl app.inline-edit]: the title edited in its heading, when
				     the details would only repeat it. -->
				{#if titleOnly && titleKey && editingField === titleKey && drafts[titleKey]?.kind === 'TEXT'}
					{@const draft = drafts[titleKey]}
					<form
						class="title-edit"
						aria-label="Edit {akt.type.fields.find((f) => f.key === titleKey)?.name ?? 'title'}"
						onsubmit={(e) => {
							e.preventDefault();
							if (titleKey) void save(titleKey);
						}}
					>
						{#if draft.kind === 'TEXT'}
							<input
								class="title-input"
								aria-label={akt.type.fields.find((f) => f.key === titleKey)?.name ?? 'Title'}
								bind:value={draft.text}
							/>
						{/if}
						{#if !live}
							<input
								class="title-reason"
								aria-label="Reason (optional)"
								placeholder="Reason (optional)"
								bind:value={reason}
							/>
						{/if}
						<Button variant="primary" size="sm" type="submit" {busy}>Save</Button>
						<Button variant="ghost" size="sm" onclick={() => titleKey && cancelEdit(titleKey)}
							>Cancel</Button
						>
					</form>
				{:else}
					<div class="heading">
						<h1>{headingOf(akt)}</h1>
						{#if titleOnly && titleKey && editable}
							<IconButton
								label="Edit {akt.type.fields.find((f) => f.key === titleKey)?.name ?? 'title'}"
								icon={Pencil}
								size="sm"
								onclick={() => {
									if (editingField) cancelEdit(editingField);
									editingField = titleKey;
								}}
							/>
						{/if}
					</div>
				{/if}
				<!-- r[impl app.header]: its status and its result, each labelled. -->
				<dl class="badges">
					<div>
						<dt>Status</dt>
						<dd>
							<Badge tone={akt.state.tone}><span data-testid="state">{akt.state.name}</span></Badge>
						</dd>
					</div>
					{#if akt.outcome}
						<div>
							<dt>Result</dt>
							<dd>
								<span data-testid="outcome"><OutcomeBadge outcome={akt.outcome} /></span>
								{#if ready}<span class="ready">{ready.passed} of {ready.of} passed</span>{/if}
							</dd>
						</div>
					{/if}
					{#if akt.archived}<div>
							<dd><Badge><Archive size={12} aria-hidden={true} /> Archived</Badge></dd>
						</div>{/if}
				</dl>
				{#if live}
					<LivePresence {live} />
				{/if}
				{#if akt.version}
					<p class="origin" role="status">
						Version {akt.version.number}{akt.version.label ? ` (${akt.version.label})` : ''},
						released
						{new Date(akt.version.releasedAt).toLocaleDateString()}: not the living version.
					</p>
				{/if}
				{#if akt.copiedFrom}
					<p class="origin">
						Copied from <a href={resolve('/akt/[number]', { number: akt.copiedFrom.number })}
							>{akt.copiedFrom.title ?? akt.copiedFrom.number}</a
						>
					</p>
				{/if}
			</div>
			<div class="actions">
				{#each pdfs as pdf (pdf.key)}
					<!-- The server's PDF route, not a page of the app. -->
					<Button href={pdf.href} download="">{pdf.label}</Button>
				{/each}
				{#if akt.type.viewerMay.create}
					<Button
						variant={actions.primary?.kind === 'copy' ? 'primary' : 'secondary'}
						disabled={busy || atVersion !== null}
						onclick={() => {
							refusal = null;
							copyOpen = true;
						}}><Copy size={14} aria-hidden={true} /> Make a copy</Button
					>
				{/if}
				{#each actions.others as transition (transition.key)}
					{@const blocked = outcomeRefusal(transition, akt.outcome)}
					<Button
						disabled={busy || blocked !== null}
						aria-describedby={blocked ? `blocked-${transition.key}` : undefined}
						onclick={() => begin(transition)}>{transition.name}</Button
					>
				{/each}
				{#if primaryTransition}
					{@const blocked = outcomeRefusal(primaryTransition, akt.outcome)}
					<Button
						variant="primary"
						disabled={busy || blocked !== null}
						aria-describedby={blocked ? `blocked-${primaryTransition.key}` : undefined}
						onclick={() => begin(primaryTransition)}>{primaryTransition.name}</Button
					>
				{:else if actions.primary?.kind === 'create'}
					<Button variant="primary" href={actions.primary.href}>
						<Plus size={14} aria-hidden={true} /> New {actions.primary.name}
					</Button>
				{/if}
				{#if !akt.archived && !atVersion && (akt.type.viewerMay.edit || akt.type.viewerMay.archive)}
					<!-- r[impl app.archive]: Archive, in the akt's menu. -->
					<Menu
						label="More actions for {akt.number}"
						items={[
							...(akt.type.viewerMay.edit
								? [
										{
											label: 'Add to…',
											icon: Link2,
											disabled: busy,
											onSelect: () => (attachOpen = true)
										}
									]
								: []),
							...(akt.type.viewerMay.archive
								? [
										{
											separated: akt.type.viewerMay.edit,
											label: 'Archive…',
											icon: Archive,
											disabled: busy,
											onSelect: () => beginArchiving('archive')
										}
									]
								: [])
						]}
					/>
				{/if}
				<!-- r[impl app.transitions]: a transition the outcome doesn't allow, unavailable, saying what it allows. -->
				{#each [...actions.others, ...(primaryTransition ? [primaryTransition] : [])] as transition (transition.key)}
					{@const blocked = outcomeRefusal(transition, akt.outcome)}
					{#if blocked}
						<p class="blocked" id="blocked-{transition.key}">
							{transition.name}: {blocked.toLowerCase()}
						</p>
					{/if}
				{/each}
			</div>
		</header>

		{#if archived}
			<!-- r[impl app.archive]: when, by whom or which rule, and why. -->
			<section class="archived" aria-label="Archived">
				<Archive size={20} aria-hidden={true} />
				<div class="archived-text">
					<p class="archived-title" data-testid="archived">{archived.title}</p>
					{#if archived.reason}<p class="archived-reason">“{archived.reason}”</p>{/if}
					<p class="archived-hint">It is read-only until it is unarchived.</p>
				</div>
				{#if !atVersion && akt.type.viewerMay.archive}
					<Button disabled={busy} onclick={() => beginArchiving('unarchive')}
						><ArchiveRestore size={14} aria-hidden={true} /> Unarchive</Button
					>
				{/if}
			</section>
		{/if}

		<div class="columns">
			<div class="main">
				{#if !titleOnly}<Card title="Details">
						<dl class="fields">
							{#each akt.type.fields as field (field.key)}
								{#if field.key === fileField}
									{#if showsFile}
										<ReleasedPdf
											number={akt.number}
											name={field.name}
											title={akt.title ?? akt.number}
											version={shownVersion}
										/>
									{/if}
								{:else if showsFile && field.kind === 'MARKDOWN'}
									<!-- The version is its PDF, not its text. -->
								{:else if !givenIn(akt.type.document, field.key, akt.state.key) && !akt.fields.some((v) => 'field' in v && v.field === field.key)}
									<!-- r[impl akt.field.states]: not yet given, and not in its states. -->
								{:else}
									<FieldRow
										{field}
										value={akt.fields.find((v) => 'field' in v && v.field === field.key)}
										bind:draft={drafts[field.key]}
										bind:reason
										{people}
										editing={editingField === field.key}
										editable={editable && givenIn(akt.type.document, field.key, akt.state.key)}
										drafted={drafted.includes(field.key)}
										{busy}
										error={editingField === field.key && refusal ? refusal.message : undefined}
										onEdit={() => {
											if (editingField) cancelEdit(editingField);
											editingField = field.key;
										}}
										onSave={() => save(field.key)}
										onCancel={() => cancelEdit(field.key)}
										onDraft={() => draft(field.key)}
										members={memberNames}
										number={akt.number}
										judged={ownVerdicts}
										judging={judgingOwn}
										onVerdict={(verdict) => giveVerdict(akt, verdict)}
										liveDraft={Boolean(live)}
										live={live && isLiveText(field) ? live : null}
									/>
								{/if}
							{/each}
						</dl>
					</Card>{/if}

				{#if contained.length > 0}
					<section class="members" aria-labelledby="members-title">
						<header>
							<h2 id="members-title">Contents</h2>
							<!-- r[impl app.archive]: how many members are archived, and showing them. -->
							{#if akt.archivedMemberCount > 0 || showArchived}
								<div class="archived-members">
									{#if akt.archivedMemberCount > 0}<span data-testid="archived-members"
											>{archivedCount(akt.archivedMemberCount)}</span
										>{/if}
									{#if !akt.archived}
										<label>
											<input
												type="checkbox"
												checked={showArchived}
												onchange={(e) => toggleArchivedMembers(e.currentTarget.checked)}
											/>
											Show archived
										</label>
									{/if}
								</div>
							{/if}
							{#if editable && parent}
								<!-- r[impl app.pull]: several at once, from its parent board. -->
								<Button size="sm" onclick={() => (pullOpen = true)}>Add from {parent.title}</Button>
							{/if}
							{#if editable}
								<Menu
									label="Add items"
									text="Add"
									items={[
										// r[impl app.permitted]: new only of the types the viewer may create.
										...contained
											.filter((key) => creatableKeys.has(key))
											.map((key) => ({
												label: `New ${typeNames[key] ?? key}`,
												icon: Plus,
												onSelect: () => {
													location.assign(newMemberHref(key));
												}
											})),
										{
											label: `Existing ${containedWords}…`,
											icon: Search,
											separated: true,
											onSelect: () => (addOpen = true)
										}
									]}
								/>
							{/if}
						</header>
						{#if sectionItems.length > 0}
							<Sections
								items={sectionItems}
								members={allMembers}
								{me}
								{busy}
								{opened}
								judging={judgingHere}
								onVerdict={(member, verdict) => giveVerdict(member, verdict)}
							/>
						{/if}
						{#if sectionItems.length === 0 || unplacedColumns.some((c) => c.akter.length > 0)}
							{#if sectionItems.length > 0}<h3 class="others">Not in a section</h3>{/if}
							<Members
								columns={unplacedColumns}
								judging={judgingHere}
								document={akt.type.document}
								{contained}
								{editable}
								movable={!akt.archived}
								{busy}
								onMove={move}
								onRemove={removeMember}
								onInto={into}
								{me}
								onVerdict={(member, verdict) => giveVerdict(member, verdict)}
								{creatable}
								onQuickAdd={quickAdd}
							/>
						{/if}
					</section>
				{/if}
			</div>

			<aside class="side">
				<!-- r[impl app.members]: where it is, only when it is somewhere. -->
				{#if akt.containers.length > 0}<Card title="Part of">
						<ul class="containers" aria-label="Part of">
							{#each akt.containers as container (container.id)}
								<li>
									<span>
										<a href={resolve('/akt/[number]', { number: container.number })}
											>{container.title ?? container.number}</a
										>
										<span class="hint"
											>{container.title
												? `${container.type.name} · ${container.number}`
												: container.type.name}</span
										>
									</span>
									<!-- r[impl app.permitted]: only from a container they may edit. -->
									{#if !akt.archived && container.type.viewerMay.edit}
										<IconButton
											label="Remove from {container.number}"
											icon={X}
											size="sm"
											disabled={busy}
											onclick={() => leave(container.number)}
										/>
									{/if}
								</li>
							{/each}
						</ul>
						{#if !akt.archived && akt.containers.some((c) => c.type.viewerMay.edit)}<div
								class="container-tools"
							>
								<Button
									size="sm"
									onclick={() => {
										transferFrom = akt.containers[0].number;
										transferOpen = true;
									}}
								>
									<ArrowRightLeft size={14} aria-hidden={true} />
									{moveWords}
								</Button>
							</div>{/if}
					</Card>{/if}

				{#if versioned}
					<VersionsCard
						number={akt.number}
						versions={akt.versions}
						pdfFields={markdownFields}
						withFile={Boolean(fileField)}
						fileRequired={!writtenHere}
						reading={atVersion}
						releasable={!atVersion && !akt.archived}
						{busy}
						{refusal}
						onRelease={releaseVersion}
						onCompare={(from, to) => {
							comparing = { from, to };
							compareOpen = true;
						}}
					/>
				{/if}
				<LinksCard
					links={akt.links}
					kinds={linkKinds(akt.type.document)}
					{editable}
					{busy}
					search={(text, kind) =>
						pickable(
							client,
							text,
							[
								akt.number,
								...akt.links
									.filter((l) => l.outgoing && l.kind === kind.key)
									.map((l) => l.akt.number)
							],
							kind.to
						)}
					onAdd={addLink}
					onRemove={removeLink}
				/>
				<Card title="History">
					<HistoryList
						revisions={akt.revisions}
						{fieldNames}
						{transitionNames}
						fieldKinds={Object.fromEntries(akt.type.fields.map((f) => [f.key, f.kind]))}
						optionNames={Object.fromEntries(
							akt.type.fields.map((f) => [
								f.key,
								Object.fromEntries((f.options ?? []).map((o) => [o.key, o.name]))
							])
						)}
						versions={akt.versions}
					/>
				</Card>
			</aside>
		</div>

		{#if comparing}
			<DiffDialog
				bind:open={compareOpen}
				number={akt.number}
				from={comparing.from}
				to={comparing.to}
				fields={akt.type.fields}
			/>
		{/if}
		{#if pending}
			<TransitionDialog
				bind:open={transitionOpen}
				name={pending.name}
				confirm={confirmationOf(
					headingOf(akt),
					akt.type.states.find((s) => s.key === pending?.to)
				)}
				fields={required(pending)}
				bind:drafts
				bind:reason
				{people}
				{refusal}
				{busy}
				onPerform={perform}
			/>
		{/if}

		<CopyDialog
			bind:open={copyOpen}
			number={akt.number}
			items={sectionItems}
			members={memberNames}
			{busy}
			error={copyOpen && refusal ? refusal.message : undefined}
			places={copyPlaces}
			onCopy={copy}
		/>

		<Dialog
			bind:open={archiveOpen}
			title={archiving === 'unarchive' ? `Unarchive ${akt.number}` : `Archive ${akt.number}`}
			description={archiving === 'unarchive'
				? 'It is live again: shown in lists, boards and search, and editable.'
				: 'It is kept, read-only, and left out of lists, boards and search unless they show archived akter.'}
		>
			<form
				id="archive-form"
				class="archive-form"
				aria-label={archiving === 'unarchive' ? 'Unarchive' : 'Archive'}
				onsubmit={(e) => {
					e.preventDefault();
					setArchived();
				}}
			>
				<label>
					Reason <span class="optional">(optional)</span>
					<input bind:value={archiveReason} maxlength={200} />
				</label>
				{#if archiveRefusal}<p role="alert">{archiveRefusal.message}</p>{/if}
			</form>
			{#snippet footer()}
				<Button variant="primary" type="submit" form="archive-form" {busy}
					>{archiving === 'unarchive' ? 'Unarchive' : 'Archive'}</Button
				>
			{/snippet}
		</Dialog>

		<Dialog
			bind:open={attachOpen}
			title="Add to…"
			description="Find what to add it to, by number or title."
		>
			<Combobox
				inline
				label="Add to"
				placeholder="Number or title…"
				search={(text) =>
					pickable(client, text, [akt.number, ...akt.containers.map((c) => c.number)])}
				onPick={(option) => join(option.value)}
			/>
		</Dialog>

		<Dialog
			bind:open={transferOpen}
			title={moveWords}
			description="Moves it in one change, recorded here and on both."
		>
			<label class="from">
				From
				<select bind:value={transferFrom}>
					{#each akt.containers as container (container.id)}
						<option value={container.number}>{container.number}</option>
					{/each}
				</select>
			</label>
			<Combobox
				inline
				label="To"
				placeholder="Where to move it…"
				search={(text) =>
					pickable(client, text, [akt.number, ...akt.containers.map((c) => c.number)])}
				onPick={(option) => transfer(option.value)}
			/>
		</Dialog>

		{#if moving}
			{@const target = moving}
			<Dialog
				bind:open={movingOpen}
				title="Move {target.member.title ?? target.member.number} to {target.column}"
			>
				<div class="choices">
					{#each target.transitions as transition (transition.key)}
						<Button
							variant={target.transitions.length === 1 ? 'primary' : 'secondary'}
							{busy}
							onclick={() => moveInto(transition)}
						>
							{transition.name}{transition.requires.length > 0 ? '…' : ''}
						</Button>
					{/each}
				</div>
			</Dialog>
		{/if}

		{#if parent && editable && pullOpen}
			<!-- Mounted only while open: its board is read when asked for. -->
			<PullDialog
				bind:open={pullOpen}
				{parent}
				here={akt.number}
				type={akt.type.key}
				{contained}
				{busy}
				onPull={pull}
			/>
		{/if}

		<Dialog
			bind:open={addOpen}
			title="Add existing {containedWords}"
			description="Find it by number or title."
		>
			<Combobox
				inline
				label="Item to add"
				placeholder="Number or title…"
				search={(text) =>
					pickable(
						client,
						text,
						[akt.number, ...akt.board.columns.flatMap((c) => c.akter.map((m) => m.number))],
						contained
					)}
				onPick={(option) => addMember(option.value)}
			/>
		</Dialog>
	{:else if view.data && !view.fetching}
		<p role="alert">There is no {number}, or you can’t see it.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.akt {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		max-width: 80rem;
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4);
	}

	.identity {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		min-width: 0;
	}

	.eyebrow {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	h1 {
		overflow-wrap: anywhere;
	}

	.badges {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
		margin: 0;
	}

	.badges div {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.badges dt {
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	.badges dd {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
	}

	.ready {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	.origin {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.others {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.blocked {
		flex-basis: 100%;
		margin: 0;
		text-align: end;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.archived {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-lg);
		background: var(--bg-subtle);
	}

	.archived-text {
		flex: 1 1 14rem;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.archived-text p {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.archived-title {
		font-weight: 600;
	}

	.archived-reason,
	.archived-hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.archived-members {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-inline-start: auto;
		margin-inline-end: var(--space-3);
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.archived-members label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.archive-form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.archive-form label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.optional {
		color: var(--text-muted);
		font-weight: 400;
	}

	.archive-form [role='alert'] {
		color: var(--tone-danger-fg);
		font-size: var(--text-sm);
	}

	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(16rem, 22rem);
		gap: var(--space-5);
		align-items: start;
	}

	@media (max-width: 1099px) {
		.columns {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	/* r[impl app.narrow]: on a phone, what it holds before its details. */
	@media (max-width: 599px) {
		.main > .members {
			order: -1;
		}
	}

	.main,
	.side {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		min-width: 0;
	}

	.fields {
		margin: 0;
	}

	.members {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.members > header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.containers {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0 0 var(--space-3);
		padding: 0;
		list-style: none;
	}

	.containers li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.containers li > span {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	.container-tools {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
	}

	.from {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.title-edit {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.title-edit {
		flex-wrap: wrap;
	}

	.title-input {
		flex: 1 1 16rem;
		font-size: var(--text-xl);
		font-weight: 600;
	}

	.title-reason {
		flex: 0 1 14rem;
	}
</style>
