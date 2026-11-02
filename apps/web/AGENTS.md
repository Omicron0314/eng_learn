# Legacy Web implementation

This directory remains the executable Next.js reference during the GPUI rewrite.
New product planning is in `../../docs/plan.md`; R00–R21 are not implemented by
changing these pages alone. Preserve existing changes and merged migrations.
Use the legacy pnpm checks for maintenance here. Historical Web instructions are
in git history at commit `46f499a`; they do not define the new architecture.

<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
