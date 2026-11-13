<script lang="ts" module>
	import type { Component } from 'svelte';

	export interface MenuItem {
		label: string;
		onSelect: () => void;
		icon?: Component<{ size?: number; 'aria-hidden'?: boolean }>;
		danger?: boolean;
		disabled?: boolean;
		/** Draws a separator above it. */
		separated?: boolean;
	}
</script>

<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';

	interface Props {
		/** The trigger's accessible name, such as "More actions for ITEM-0003". */
		label: string;
		items: MenuItem[];
		/** What the trigger shows; an ellipsis by default. The label must
		 * contain it, so that people who name what they see reach it. */
		text?: string;
	}

	let { label, items, text }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="menu-trigger {text ? 'text' : 'icon'}"
		aria-label={label}
		title={label}
	>
		{#if text}{text}{:else}<Ellipsis size={16} aria-hidden={true} />{/if}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content class="menu-content" sideOffset={4} align="end">
			{#each items as item, i (i)}
				{#if item.separated}<DropdownMenu.Separator class="menu-separator" />{/if}
				<DropdownMenu.Item
					class="menu-item {item.danger ? 'danger' : ''}"
					disabled={item.disabled}
					onSelect={item.onSelect}
				>
					{#if item.icon}<item.icon size={14} aria-hidden={true} />{/if}
					{item.label}
				</DropdownMenu.Item>
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
	:global(.menu-trigger) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-1);
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-muted);
		font: inherit;
		cursor: pointer;
	}

	:global(.menu-trigger.icon) {
		width: 2rem;
		height: 2rem;
	}

	:global(.menu-trigger.text) {
		min-height: 2.25rem;
		padding: 0 var(--space-3);
		border-color: var(--border-strong);
		background: var(--surface);
		color: var(--text);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	:global(.menu-trigger:hover) {
		background: var(--bg-muted);
		color: var(--text);
	}

	:global(.menu-content) {
		z-index: 50;
		min-width: 12rem;
		padding: var(--space-1);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
	}

	:global(.menu-item) {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
		color: var(--text);
		cursor: pointer;
		user-select: none;
	}

	:global(.menu-item[data-highlighted]) {
		background: var(--bg-muted);
	}

	:global(.menu-item.danger) {
		color: var(--tone-danger-fg);
	}

	:global(.menu-item[data-disabled]) {
		opacity: 0.5;
		cursor: not-allowed;
	}

	:global(.menu-separator) {
		height: 1px;
		margin: var(--space-1) 0;
		background: var(--border);
	}
</style>
