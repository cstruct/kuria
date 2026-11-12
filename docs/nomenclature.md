# Nomenclature

This document defines the preferred language for Kuria specs, code, API surfaces, and documentation.

Terms are introduced in dependency order: each definition only uses terms defined above it, or points explicitly to a later section. If a new term becomes important to the project, add it here before spreading it across specs or APIs.

## Systems and boundaries

- `Kuria`
  - The case and record management system defined by this repository: issues, nested work, test protocols and records, release records, and CAPA.
  - Kuria hardcodes as little as possible about what these things are. Their kinds and the rules they follow come from configuration at runtime (see Akter).

- `Konklav`
  - The identity and access service Kuria relies on. Kuria is a Konklav `relying system`: Konklav decides who someone is and what they may do, and Kuria enforces those decisions on its own data.
  - Terms Kuria borrows from Konklav, such as `tenant` and `subject`, keep the meaning Konklav's nomenclature gives them.

## Akter

- `akt` (plural `akter`)
  - The one generic object in Kuria. A bug report, an epic, a test record, a release record, and a CAPA are each an akt.
  - From the Swedish *akt*, a case file: an akt is something that is opened, worked on, and kept as a record.
  - Kuria's core knows only akter. What distinguishes one akt from another is its `akt type`.
  - Prefer this term over `issue`, `ticket`, `item`, or `object` when speaking of the generic thing. Those words may name an akt type, never the generic concept.
  - Identifiers stay singular: tables `akt` and `akt_type`, paths `/akt` and `/akt/{id}`. The plural `akter` appears only in prose.

- `akt type`
  - A runtime definition of one kind of akt, such as `bug`, `epic`, or `test record`.
  - Every akt has exactly one akt type.
