# Atogaki development workflow

## Start of work

1. Read `docs/product-direction.md`, `docs/roadmap.md`, and relevant decision records.
2. Inspect `git status` before editing; preserve unrelated work already in the tree.
3. After a handoff, context compaction, or uncertainty about current progress, recover context from `git log`, `git status`, the relevant diff, and `docs/` rather than relying on conversation memory.

## Implementation and verification

- Keep commits small and cohesive. Run the relevant regression checks before each commit; at minimum run `cargo fmt --check`, `git diff --check`, and applicable tests.
- If dependency downloads fail with DNS errors or sustained low-speed timeouts, retry from interactive zsh after running the existing `proxy_on` helper from `.zshrc`. Never record or print proxy credentials or endpoints in project files or logs.
- If a check is blocked by the environment, record the command, blocker, and remaining risk in the handoff.
- Stage only files belonging to the current change. Do not include pre-existing unrelated changes in a commit.
- Push a completed cohesive milestone after its commit, unless the remote is unavailable or the user asks otherwise. Never force-push without explicit approval.

## Documentation and decisions

- Update `docs/roadmap.md` for material progress, scope shifts, and newly discovered technical debt.
- Update `docs/product-direction.md` before changing target users, product priority, storage strategy, or the primary platform.
- Add a dated record in `docs/decisions/` for durable architectural choices with meaningful alternatives.
- Raise possible new product scenarios, meaningful expansion opportunities, and consequential design choices with the user before committing to them.

## Handoff

At the end of a coherent work segment, report the design structure, implementation highlights, verification results, remaining risks, and the recommended next direction. Link to the changed files.
