<script lang="ts">
	// r[impl app.type-identity]
	import { markOf, type Marked } from './identity';

	interface Props {
		type: Marked & { name: string };
		/** Shows the type's name beside its mark. */
		named?: boolean;
		size?: 'sm' | 'md';
		/** Whether a long name wraps onto a second line, rather than being
		 * cut short. */
		wrap?: boolean;
	}

	let { type, named = false, size = 'sm', wrap = false }: Props = $props();
	const mark = $derived(markOf(type));
</script>

<span class="type-mark {size}" class:wrap>
	<!-- A class per colour, not an inline style, which the CSP forbids. -->
	<span class="square {mark.colour}" aria-hidden="true">
		{#if mark.icon}<mark.icon size={size === 'sm' ? 12 : 16} aria-hidden={true} />{:else}<span
				class="initial">{type.name.slice(0, 1)}</span
			>{/if}
	</span>
	{#if named}<span class="name">{type.name}</span>{/if}
</span>

<style>
	.type-mark {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.square {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-sm);
	}

	.square.neutral {
		background: var(--type-neutral-bg);
		color: var(--type-neutral-fg);
	}

	.square.blue {
		background: var(--type-blue-bg);
		color: var(--type-blue-fg);
	}

	.square.green {
		background: var(--type-green-bg);
		color: var(--type-green-fg);
	}

	.square.amber {
		background: var(--type-amber-bg);
		color: var(--type-amber-fg);
	}

	.square.red {
		background: var(--type-red-bg);
		color: var(--type-red-fg);
	}

	.square.purple {
		background: var(--type-purple-bg);
		color: var(--type-purple-fg);
	}

	.square.teal {
		background: var(--type-teal-bg);
		color: var(--type-teal-fg);
	}

	.square.pink {
		background: var(--type-pink-bg);
		color: var(--type-pink-fg);
	}

	.sm .square {
		width: 1.25rem;
		height: 1.25rem;
	}

	.md .square {
		width: 1.75rem;
		height: 1.75rem;
	}

	.initial {
		font-size: 0.7rem;
		font-weight: 600;
		line-height: 1;
	}

	.md .initial {
		font-size: var(--text-sm);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.wrap .name {
		white-space: normal;
		overflow-wrap: anywhere;
	}
</style>
