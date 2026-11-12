# Repository Guidance

- Read [docs/nomenclature.md](docs/nomenclature.md) before changing specs, the API, or documentation, and use its terms in code, tests and docs. A new project term goes there first.
- Requirements live in [docs/spec/](docs/spec/README.md) as Tracey rules (`r[...]`). Write or update the rule before the code, and reference it from code and tests as `r[impl ...]` and `r[verify ...]`. `mise run lint` validates the references.
- Design and rationale live in [docs/architecture.md](docs/architecture.md); what Kuria has to support is in [docs/requirements.md](docs/requirements.md).
- Each software item under `items/` has a README covering its responsibilities, entry points, running, testing and traceability. Keep it current when the item changes.
- Kuria is a Konklav relying system. Authenticate and authorize through Konklav's relying crates (`konklav-axum`, `konklav-relying-client`), never with token handling of its own.
- Run `mise run lint` and `mise run test` before committing.
