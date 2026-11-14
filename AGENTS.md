# Repository Guidance

- Follow [CONTRIBUTING.md](CONTRIBUTING.md) for setting up, checking a change and CI.
- Read [docs/nomenclature.md](docs/nomenclature.md) before changing specs, the API, or documentation, and use its terms in code, tests and docs. A new project term goes there first.
- Requirements live in [docs/spec/](docs/spec/README.md) as Tracey rules (`r[...]`). Write or update the rule before the code, and reference it from code and tests as `r[impl ...]` and `r[verify ...]`. `mise run lint` validates the references.
- Design and rationale live in [docs/architecture.md](docs/architecture.md); what Kuria has to support is in [docs/requirements.md](docs/requirements.md).
- Each software item under `items/` has a README covering its responsibilities, entry points, running, testing and traceability. Keep it current when the item changes.
- Kuria is a Konklav relying system. Authenticate and authorize through Konklav's relying crates (`konklav-axum`, `konklav-relying-client`), never with token handling of its own.
- Plans live in [plans/](plans/), an akt directory of Markdown files that the `kuria` CLI reads and writes without a server: `mise run plans -- <command>`, with `mise run plans -- guide` explaining it. For complex features, substantial refactors, or multi-step migrations, create or update a `plan` there, with its tasks, gaps and decisions, following [.agents/plans/PLANS.md](.agents/plans/PLANS.md), before implementation. Read the active plans with `mise run plans -- ls plan --state active`, take a task (`set <task> assignee=@me`, `do <task> start`), record progress on it, finish it with a reason, and commit each change with the reason it gives back. Change plans through the CLI rather than by hand, and don't start Markdown plans of another kind.
- Run `mise run lint` and `mise run test` before committing.
