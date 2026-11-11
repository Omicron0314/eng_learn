# AGENTS.md

Cijian is migrating to a desktop English-learning app built with Rust, GPUI and
gpui-kit-material. A Rust workspace and desktop application now exist;
implementation and acceptance are in progress. Web/WASM, native mobile and
cross-device sync are long-term planning only and are not current delivery
scope. The legacy TypeScript/Next.js app remains available until migration
acceptance. See `docs/IMPLEMENTATION.md` and decision D47.

## Authority and scope

- Read `docs/plan.md`, the relevant domain/UI document, and the task spec first.
- User requirements override the final rendered `docs/cijian-m3.html`, which
  overrides current specifications, which override legacy implementation/docs.
- The design's script moves generation and story lists into Articles. Inspect
  the rendered design; do not implement its obsolete initial Today markup.
- `docs/neko.html` is a secondary visual reference. Preserve both HTML originals.
- Current tasks are R00–R17 in `docs/BACKLOG.md`; R18–R21 (Web/WASM, native
  mobile, sync) are deferred as long-term planning and must not be started
  without a new explicit scope decision. Legacy T/S tasks and evidence
  remain in git history at commit `46f499a` and impose no current approval workflow.
- The user subsequently authorized actual Rust implementation. The previous
  documentation-only restriction is historical; platform acceptance still needs
  evidence and must not be inferred from compilation or legacy tests.
- Preserve pre-existing worktree changes. Do not stage, commit, publish, access
  secrets, make paid calls, or operate on real user data without matching scope.

## Current commands (legacy Web)

```sh
pnpm install --frozen-lockfile
pnpm dev
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm e2e
pnpm report:experiment
```

`pnpm eval:generation --allow-paid` makes real calls and requires explicit
paid-call authorization. Never run it as routine validation. Keep dev logs
outside the repository to avoid Turbopack refresh loops.

Rust commands are available through the root workspace and pinned toolchain.
Use `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace`. GPUI tests need the test-support feature where
specified in `docs/TESTING.md`. R00 pins the Kit/Material revisions and root patches;
its outstanding platform checks remain acceptance limitations.

## Target boundaries

- Domain is pure Rust: no GPUI, DB, network, filesystem, platform clock or RNG.
  Inject time, randomness and identifiers.
- UI calls application commands/queries. Repositories own persistence;
  provider adapters own LLM/TTS calls; platform adapters own device facilities.
- Use the gpui-kit facade for matching GPUI types; do not mix engine versions.
  Material supplies visuals/motion and Kit supplies behavior/focus/accessibility.
- Validate all boundary input and model output. Deserialization is not validation.
- Business writes include events in the same transaction; ratings also append
  review_logs. Logs/cache/projections have the exceptions in DATA_MODEL.
- Commands use operation IDs and revisions. Network IO is outside DB transactions.
- All actual model calls, including failures/repairs, are logged. No hidden SDK
  retries; model, endpoint and voice names belong in configuration only.
- Missing AI configuration must not block offline study. No production fake mode.
- Secrets go through platform SecretStore, never events, logs or backup packages.
- Tests use test-only doubles and original fixtures. Do not distribute user decks.

## Legacy boundaries during migration

- `packages/core` remains IO-free. Services access DB through repositories.
- Test doubles are exported only by `@eng-learn/providers/testing`.
- Leave merged Drizzle migrations unchanged. The Rust store has its own versioned
  migrations and reads the legacy store only through an explicit conversion path.
- `prompts/*.v1.md` are runtime assets, not obsolete planning docs. Preserve them
  until a scoped semantic change adds a version and before/after evaluation.

## Delivery

- Verification evidence comes from CI on Linux. Local runs are for iteration
  and debugging, not acceptance.
- English identifiers/AGENTS/commit messages; Chinese UI and product docs.
- Follow task dependencies; one schema writer at a time. Keep logical changes,
  dependencies, migrations and prompt versions in separate atomic commits.
- Standing authorization: after a user-assigned task is complete, commit its
  code, tests and documentation without waiting for a separate request. Use
  Conventional Commits and include `Task: Rxx`.
- Record implementation choices in `docs/DECISIONS.md`; ordinary reversible
  choices do not require the archived per-spec approval process.
- Do not add a project LICENSE or package license field; licensing is deferred.
- Update task status only with matching evidence. Legacy test counts do not prove
  Rust correctness; WASM/mobile layouts do not prove native mobile support.
- Run checks appropriate to changed files. Documentation-only work needs link,
  task/dependency and design-coverage checks, not invented product test claims.
- Local runs are for fast feedback; CI is the evidence. Every CI step must be
  reproducible locally with one documented command, and CI must not hold logic
  that exists only in YAML.
- After every commit that changes desktop code, run `tools/build-desktop.sh`
  (debug profile by default) so `~/.local/bin/cijian-desktop` and the launcher
  entry point at the build containing that commit. A rebuild at the same path is
  enough; rerun the script only after `cargo clean` or a target-dir change.
  Verify with `realpath ~/.local/bin/cijian-desktop` and the binary's mtime.
- UI completion needs behavior and light/dark/responsive evidence produced by CI
  on Linux. Windows/macOS, real IME, touch, screen readers, human listening and
  paid real-model quality are out of scope and must not be restored as ACs.
