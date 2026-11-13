# Repository Guidance

- Read [docs/nomenclature.md](docs/nomenclature.md) before changing specs, the API, or documentation, and use its terms in code, tests and docs. A new project term goes there first.
- Requirements live in [docs/spec/](docs/spec/README.md) as Tracey rules (`r[...]`). Write or update the rule before the code, and reference it from code and tests as `r[impl ...]` and `r[verify ...]`. `mise run lint` validates the references.
- Design and rationale live in [docs/architecture.md](docs/architecture.md); what Kuria has to support is in [docs/requirements.md](docs/requirements.md).
- Each software item under `items/` has a README covering its responsibilities, entry points, running, testing and traceability. Keep it current when the item changes.
- Kuria is a Konklav relying system. Authenticate and authorize through Konklav's relying crates (`konklav-axum`, `konklav-relying-client`), never with token handling of its own.
- Plans live in Kuria, on the local dogfooding stack: `mise run dogfood:serve` keeps it running with its data ([dogfood/README.md](dogfood/README.md)), and `mise run dogfood:kuria -- guide` explains the CLI. For complex features, substantial refactors, or multi-step migrations, create or update a `plan` there, with its tasks, gaps and decisions, following [.agents/plans/PLANS.md](.agents/plans/PLANS.md), before implementation. Read the active plans with `mise run dogfood:kuria -- ls plan --state active`, take a task, record progress on it, and finish it with a reason. Don't start new Markdown plans.
- Run `mise run lint` and `mise run test` before committing.
