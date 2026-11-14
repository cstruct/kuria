<script lang="ts">
	// r[impl app.verdicts]: an outcome, wherever the app shows one.
	import Badge from '#lib/ui/Badge.svelte';
	import { shownOutcome, type Outcome } from './outcomes';

	let { outcome }: { outcome: Outcome } = $props();
	const shown = $derived(shownOutcome(outcome));
</script>

<Badge tone={shown.tone}><span data-outcome={outcome}>{shown.text}</span></Badge>
