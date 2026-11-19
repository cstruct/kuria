<script lang="ts">
	import { initialsOf } from '#lib/akt/people.ts';

	interface Props {
		/** The person's name, as shown beside it. */
		name: string;
		size?: 'sm' | 'md';
	}

	let { name, size = 'sm' }: Props = $props();
	/** A hue picked from the name, so that each person keeps theirs. */
	const tone = $derived(
		['blue', 'green', 'amber', 'purple', 'teal', 'pink'][
			[...name].reduce((sum, c) => sum + c.charCodeAt(0), 0) % 6
		]
	);
</script>

<!-- Initials, beside the name that says who it is: decoration only. -->
<span class="avatar {size} {tone}" aria-hidden="true">{initialsOf(name)}</span>

<style>
	.avatar {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border-radius: 999px;
		font-weight: 600;
		line-height: 1;
		letter-spacing: 0.02em;
		background: var(--type-bg);
		color: var(--type-fg);
	}

	.sm {
		width: 1.375rem;
		height: 1.375rem;
		font-size: 0.625rem;
	}

	.md {
		width: 1.75rem;
		height: 1.75rem;
		font-size: var(--text-xs);
	}

	.blue {
		--type-bg: var(--type-blue-bg);
		--type-fg: var(--type-blue-fg);
	}

	.green {
		--type-bg: var(--type-green-bg);
		--type-fg: var(--type-green-fg);
	}

	.amber {
		--type-bg: var(--type-amber-bg);
		--type-fg: var(--type-amber-fg);
	}

	.purple {
		--type-bg: var(--type-purple-bg);
		--type-fg: var(--type-purple-fg);
	}

	.teal {
		--type-bg: var(--type-teal-bg);
		--type-fg: var(--type-teal-fg);
	}

	.pink {
		--type-bg: var(--type-pink-bg);
		--type-fg: var(--type-pink-fg);
	}
</style>
