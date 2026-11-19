<script lang="ts" module>
	/** One thing a combobox offers. */
	export interface Option {
		value: string;
		label: string;
		/** A second line, such as a number or a type. */
		hint?: string;
	}
</script>

<script lang="ts">
	import { Combobox } from 'bits-ui';
	import Search from '@lucide/svelte/icons/search';

	interface Props {
		/** The input's accessible name. */
		label: string;
		placeholder?: string;
		/** The options for what has been typed. */
		search: (text: string) => Promise<Option[]>;
		/** Called with the option picked. */
		onPick: (option: Option) => void;
		/** Shown when nothing matches. */
		empty?: string;
		/** Whether the options show in the page's flow, under the input,
		 * as in a dialog, rather than floating over what is below it. */
		inline?: boolean;
	}

	let {
		label,
		placeholder,
		search,
		onPick,
		empty = 'Nothing matches.',
		inline = false
	}: Props = $props();

	let text = $state('');
	let options = $state<Option[]>([]);
	let open = $state(false);
	let value = $state('');
	let searching = $state(false);
	let latest = 0;

	// Searched as the person types, a little after they pause, and only the
	// latest answer shown.
	$effect(() => {
		const typed = text;
		const asked = ++latest;
		if (!typed.trim()) {
			options = [];
			return;
		}
		searching = true;
		const timer = setTimeout(async () => {
			const found = await search(typed).catch(() => []);
			if (asked === latest) {
				options = found;
				searching = false;
				open = true;
			}
		}, 150);
		return () => clearTimeout(timer);
	});

	function pick(next: string) {
		const option = options.find((o) => o.value === next);
		if (!option) return;
		onPick(option);
		text = '';
		value = '';
		options = [];
	}
</script>

{#snippet found()}
	{#each options as option (option.value)}
		<Combobox.Item class="combobox-item" value={option.value} label={option.label}>
			<span class="label">{option.label}</span>
			{#if option.hint}<span class="hint">{option.hint}</span>{/if}
		</Combobox.Item>
	{:else}
		<p class="combobox-empty">{searching ? 'Searching…' : empty}</p>
	{/each}
{/snippet}

<!-- r[impl app.pickers]: emptied once something is picked. -->
<Combobox.Root
	type="single"
	bind:open
	bind:value
	onValueChange={pick}
	items={options}
	inputValue={text}
>
	<div class="combobox">
		<Search size={14} aria-hidden={true} />
		<Combobox.Input
			class="combobox-input"
			aria-label={label}
			{placeholder}
			oninput={(e) => (text = e.currentTarget.value)}
		/>
	</div>
	{#if inline}
		<!-- In a dialog: under the input, inside the dialog, which scrolls. -->
		<Combobox.ContentStatic class="combobox-content combobox-inline">
			{@render found()}
		</Combobox.ContentStatic>
	{:else}
		<Combobox.Portal>
			<Combobox.Content class="combobox-content" sideOffset={4}>
				{@render found()}
			</Combobox.Content>
		</Combobox.Portal>
	{/if}
</Combobox.Root>

<style>
	.combobox {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-left: var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text-muted);
	}

	.combobox:focus-within {
		outline: 2px solid var(--focus);
		border-color: var(--focus);
	}

	.combobox :global(.combobox-input) {
		border: 0;
		outline: none;
		padding-left: 0;
		background: transparent;
	}

	:global(.combobox-content) {
		z-index: 50;
		min-width: var(--bits-combobox-anchor-width);
		max-height: 20rem;
		overflow: auto;
		padding: var(--space-1);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
	}

	:global(.combobox-item) {
		display: flex;
		flex-direction: column;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	:global(.combobox-item[data-highlighted]) {
		background: var(--bg-muted);
	}

	.label {
		font-size: var(--text-sm);
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--text-muted);
		font-family: var(--font-mono);
	}

	.combobox-empty {
		padding: var(--space-2) var(--space-3);
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	:global(.combobox-inline) {
		position: static;
		width: 100%;
		margin-top: var(--space-1);
	}
</style>
