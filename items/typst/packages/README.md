# Bundled typst packages

The only packages a diagram block or a PDF can import (`pdf.packages`,
PLAN-015-D05), compiled into Kuria and resolved by `kuria-typst`'s `World`
alone. Each is as published at `packages.typst.org`, unmodified, with its
own license:

| Package | Version | License | Why |
|---|---|---|---|
| fletcher | 0.5.8 | MIT | Diagrams with nodes and arrows |
| cetz | 0.3.4 | LGPL-3.0-or-later | fletcher's own import |
| cetz | 0.5.2 | LGPL-3.0-or-later | Drawing on a canvas; cetz-plot's import |
| cetz-plot | 0.1.4 | LGPL-3.0-or-later | Plots and charts |
| oxifmt | 0.2.1 | MIT-0 | cetz 0.3.4's import, for formatting numbers |
| oxifmt | 1.0.0 | MIT OR Apache-2.0 | cetz 0.5.2's import |

cetz 0.5.2 includes `cetz-core/cetz_core.wasm`, a typst plugin built from
the cetz repository's `cetz-core` crate.

To update one, replace its directory with the published package, keeping it
unmodified, and change its version where `kuria-typst` imports it.
