# Kuria's logo

| File | What it is |
|---|---|
| [`kuria-logo.svg`](kuria-logo.svg) | The mark and the name, the name dark on light and light on dark (`prefers-color-scheme`) |
| [`kuria-mark.svg`](kuria-mark.svg) | The mark alone: a document with its corner folded and a K, for icons and favicons |
| [`kuria-logo.png`](kuria-logo.png), [`kuria-mark-512.png`](kuria-mark-512.png) | The same as PNGs, for where SVG isn't taken |

The mark is blue `#2f56c8`, the app's accent, its fold `#a9bdf3` and its K
white, in either theme; the name is drawn in strokes, not set in a font,
so it looks the same everywhere, `#172033` on light and `#e6e9ee` on dark.

In the app the mark is the favicon (`items/web/src/lib/assets/favicon.svg`,
with `items/web/static/apple-touch-icon.png` at 180 pixels), and the
sidebar draws the logo inline (`items/web/src/lib/shell/Logo.svelte`), its
name in the theme's text colour. Change the SVGs here and those together;
the PNGs are made from them with `rsvg-convert`:

```sh
rsvg-convert -h 128 docs/brand/kuria-logo.svg -o docs/brand/kuria-logo.png
rsvg-convert -w 512 docs/brand/kuria-mark.svg -o docs/brand/kuria-mark-512.png
rsvg-convert -w 180 docs/brand/kuria-mark.svg -o items/web/static/apple-touch-icon.png
```
