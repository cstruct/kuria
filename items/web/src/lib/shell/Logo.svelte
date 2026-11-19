<script lang="ts">
	// Kuria's logo, its mark and its name, drawn inline so that the name
	// takes the theme's text colour (docs/brand/kuria-logo.svg).
	let { height = 24 }: { height?: number } = $props();
</script>

<svg class="logo" viewBox="0 0 217 64" {height} role="img" aria-label="Kuria">
	<path
		d="M14 4H42L60 22V50A10 10 0 0 1 50 60H14A10 10 0 0 1 4 50V14A10 10 0 0 1 14 4Z"
		class="tile"
	/>
	<path d="M42 4V17A5 5 0 0 0 47 22H60Z" class="fold" />
	<path d="M21.5 17V47M37.5 22.5L23 35M28.5 30.5L40 47" class="k" />
	<path
		class="line"
		d="M82 12V50M99 26L85 39M90.5 34.5L100 50M114 26V39A10 10 0 0 0 134 39M134 26V50M148 50V26M148 37A11 11 0 0 1 159 26H162M173 26V50M209.5 26V50"
	/>
	<circle class="line" cx="199" cy="38" r="10.5" />
	<circle class="dot" cx="173" cy="14" r="4.4" />
</svg>

<style>
	.logo {
		display: block;
		width: auto;
	}

	/* The mark keeps its own colours in either theme. */
	.tile {
		fill: #2f56c8;
	}

	.fold {
		fill: #a9bdf3;
	}

	.k,
	.line {
		fill: none;
		stroke-width: 7;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.k {
		stroke: #fff;
		stroke-width: 7.5;
	}

	.line {
		stroke: var(--text);
	}

	.dot {
		fill: var(--text);
	}
</style>
