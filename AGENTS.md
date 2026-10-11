# AGENTS.md

English-learning web app (PWA, self-use MVP): import word lists -> FSRS review -> LLM-generated
articles that reuse today's words -> tap-to-mark words -> TTS audio -> event log, stats, control experiment.

`docs/plan.md` is the single source of truth. Read the relevant split doc and the task spec in
`docs/specs/` before starting any task. Pick tasks from `docs/BACKLOG.md` in dependency order.

Status: MVP implementation authorized. The user approved all existing M0–M6 specs and delegated
pending decisions to the agent for a no-interruption one-shot build. See `docs/MVP-EXECUTION.md`.
This supersedes the stop-and-ask rules for implementation choices, dependencies, new migrations,
and prompt implementation. Do not alter merged migrations, use secrets, make paid calls, or publish.

## Commands

```
pnpm install
pnpm dev                 # Next.js dev server
pnpm test                # Vitest unit tests
pnpm lint
pnpm typecheck
pnpm e2e                 # Playwright
pnpm eval:generation     # MANUAL ONLY: real LLM calls, cost cap (~$1)
pnpm report:experiment   # exposed vs control Again rate + sample size
docker compose up        # single container + data volume; set APP_PASSWORD
```

## Layout

```
packages/core       pure logic: lexicon, analyzer, knowledge, srs, generation
packages/providers  LLM / TTS / BlobStore interfaces, implementations, fakes
packages/db         Drizzle schema, migrations, repository layer
apps/web            Next.js App Router pages + API routes
prompts/            versioned prompts (article.v1.md, ...)
fixtures/           test samples, LLM record/replay
docs/specs/         one spec per task (Txx-*.md); docs/spikes/ spike results (Sx.md)
```

## Architecture boundaries

- `packages/core` does no IO (no fetch, fs, db). Inject dependencies via interfaces;
  never import db or provider implementations.
- All LLM/TTS calls go through `packages/providers`. Tests use fakes by default
  (FakeLLM, FakeTTS, MemoryBlob).
- Model names, voice names and provider endpoints live only in config (`.env` / settings),
  never in code, prompts, tests or docs. `.env.example` holds placeholders only.
- `LLM_BASE_URL`, `LLM_API_KEY`, `LLM_MODEL` are required; fail fast with a clear error
  if missing. No default model.
- Validate all LLM output and API input with zod. On LLM validation failure retry once with
  a repair prompt, then return a clear error and record it in `llm_calls`.
- Every write also writes `events` (reviews also write `review_logs`). Both tables are append-only.
- Business code accesses the DB only through the repository layer in `packages/db`.
- In-product AI is a deterministic workflow; the LLM only writes text and glosses.

## Data conventions

- IDs are UUIDv7. Every table carries `user_id` and `updated_at` (MVP has a single user).
- Log every LLM call to `llm_calls`; store the model name in `llm_calls.model` / `articles.model`.

## Conventions

- English for AGENTS.md, identifiers and commit messages; Chinese for product docs and UI copy.
- Prompts live in `prompts/` with a version suffix; the version goes into `articles.prompt_version`.
  Prompt changes must include a before/after eval.
- Spec first: write the spec and failing tests, then implement. One task per branch/PR, small diffs.
- Spike code never goes into the product; results go to `docs/spikes/Sx.md`.
- Only one task may change the schema at a time. Never modify merged migrations.
- Do not add a LICENSE file or a `license` field in package.json (deferred).
- When a task is done, set its status in `docs/BACKLOG.md`.

## Commits (atomic)

- One logical change per commit. Each commit must pass `pnpm lint`, `pnpm typecheck`, `pnpm test`.
- Never mix unrelated changes (feature, refactor, formatting, deps) in one commit; split them.
- Dependency changes, schema migrations and prompt changes each go in their own commit.
- Conventional Commits in English: `type(scope): summary` (feat, fix, test, refactor, docs,
  chore, build, ci). Add `Task: Txx` in the body. Never commit secrets.

## Stop and ask the human when

- an item is marked 待定 (undecided); never decide it yourself
- adding a dependency
- creating a schema migration or touching a merged one
- changing prompt semantics
- anything involving secrets, paid API calls or licenses
- specs conflict, or are too incomplete to write testable acceptance criteria

Status markers in docs: 已定 = fixed, never change; 默认 = default, may change but log it in
`docs/DECISIONS.md`; 暂缓 = deferred, ignore and don't ask.

## Definition of Done

- Every acceptance criterion in the task spec is covered by a passing test
- `pnpm test`, `pnpm lint`, `pnpm typecheck` are green
- Data writes have `events` rows with assertions
- `docs/specs/` updated; `docs/licenses.md` updated when word lists are involved

## Docs index

PRD.md (goals, scope) | ARCHITECTURE.md (stack, layout) | DATA_MODEL.md (tables) |
SPEC-CORE.md (feature specs) | BACKLOG.md (tasks, workflow) | ROADMAP.md | DECISIONS.md
