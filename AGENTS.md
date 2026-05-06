# AGENTS.md — Fullstack Template

> **Purpose**: This file is the **single source of truth for AI agents** working on this codebase. It contains architecture conventions, code style rules, quality gates, and documentation discipline — everything an agent needs to write code that matches the project's standards.
>
> **For developers**: See [README.md](./README.md) for environment setup, quick start, and endpoint references.

---

## Documentation Operating Model

**Always update the authoritative document for the thing you changed.** This repo assumes most implementation work is done by agents, so stale docs directly cause bad future changes.

### Document Ownership

| File | Owns | Do not put here |
|---|---|---|
| `README.md` | Human developer onboarding: prerequisites, local setup, run commands, endpoints, config entry points, day-one workflow | Detailed architecture rules, coding standards, agent workflows |
| `AGENTS.md` | Agent operating rules: architecture conventions, code style, quality gates, security policy, doc-update rules, local skills | Step-by-step human setup unless agents need it to validate work |
| `CONTEXT.md` | Domain vocabulary, seam names, layer names, core concepts, invariants that shape naming | Generic setup commands, implementation checklists |
| `docs/how-to-add-feature.md` | Repeatable vertical-slice workflow for adding a new domain feature | One-off decisions, project-specific incident notes |
| `docs/adr/` | Accepted architecture decisions, rejected alternatives, durable trade-offs | Temporary task plans, obvious implementation details |
| `docs/openapi.json` | Generated REST API contract from backend `utoipa` definitions | Hand-written edits |

### Change-to-Documentation Matrix

| Change made | Required documentation action |
|---|---|
| Local setup, prerequisites, ports, endpoints, `.env.example`, `local.example.yaml`, Docker profile, `mise` task that developers run | Update `README.md` |
| Agent workflow, code style, quality gate, security rule, commit rule, local skill, diagnosis/refactor/TDD convention | Update `AGENTS.md` |
| Architecture boundary, crate responsibility, folder layout, dependency direction, new seam/trait, domain vocabulary, invariant | Update `AGENTS.md`; update `CONTEXT.md` if naming/domain language changed |
| New domain feature pattern or changed vertical-slice flow | Update `docs/how-to-add-feature.md`; link from `AGENTS.md` only if the agent rule changes |
| New crate/package, dependency bump, generated artifact, feature flag, build tool, codegen command | Update `AGENTS.md`; update `README.md` if developers must run or configure it |
| Backend DTO, `#[derive(ToSchema)]`, or `#[utoipa::path]` change | Run `mise run openapi:gen`; commit updated `docs/openapi.json` and frontend generated API artifacts |
| Auth, authorization, audit, rate limiting, TLS, secrets, PII, CSP, CORS, request limits | Update `AGENTS.md`; update `README.md` only for developer-facing env/config steps |
| Big architectural decision with meaningful alternatives or long-term consequences | Add an ADR under `docs/adr/`; add a short pointer in `AGENTS.md` if agents must follow it |
| README/AGENTS/CONTEXT disagree | Treat `AGENTS.md` as the agent-rule source, `README.md` as the human setup source, and `CONTEXT.md` as the vocabulary source; fix the stale file immediately |

### Duplication Rules

- **Prefer pointers over copies**: if two docs need the same topic, one owns the details and the other links to it.
- **README stays operational**: a developer should be able to run the project from README without reading architecture rules.
- **AGENTS stays prescriptive**: an agent should know exactly which patterns, checks, and docs to update without reading README except for local commands.
- **CONTEXT stays semantic**: it should explain what terms mean, not how to run tools.
- **Generated files stay generated**: never manually edit `docs/openapi.json`, `schema.d.ts`, or generated Zod output except to fix the generator.

---

## Directory Structure

```
project-root/
├── backend/                  # Rust Cargo Workspace
│   ├── Cargo.toml            # Workspace root, shared dependencies
│   ├── rustfmt.toml          # Import ordering: std → external → workspace → super → crate
│   ├── config/
│   │   ├── default.yaml      # Production-safe defaults (checked in)
│   │   └── local.example.yaml # Local override template (gitignored)
│   └── crates/
│       ├── config/           # YAML+env config (figment). AppConfig struct.
│       ├── dto/              # Shared DTOs. Request/Response, utoipa ToSchema.
│       ├── model/            # Domain models (sqlx::FromRow).
│       ├── migration/        # Database migrations via refinery. Per-DB SQL files.
│       ├── repo/             # Repository pattern. UserRepo seam + Postgres/MSSQL adapters.
│       │   └── src/user_repo/  # Trait, adapters, adapter-specific tests, test-helpers.
│       ├── svc/              # Business logic. Depends on repo traits (not impls).
│       ├── server/           # Axum REST (3001) + tonic gRPC (50051). Combined binary.
│       └── grpc/             # Standalone gRPC server (optional).
├── frontend/                 # pnpm workspace
│   └── src/
│       ├── app/              # Next.js App Router
│       │   ├── (auth)/       # Login, OIDC callback
│       │   └── dashboard/    # Protected pages
│       ├── components/
│       │   ├── ui/           # shadcn/ui components
│       │   └── features/     # Business feature components
│       ├── lib/
│       │   ├── api/          # axios client + typed endpoints
│       │   └── auth/         # next-auth config (generic OIDC)
│       ├── schemas/          # Zod schemas + inferred TypeScript types
│       ├── hooks/            # SWR data fetching hooks
│       ├── stores/           # Zustand stores (UI state)
│       └── styles/           # Tailwind CSS globals
├── docs/
│   ├── adr/                  # Architecture Decision Records
│   └── openapi.json          # Generated OpenAPI spec (source of truth for FE types)
├── proto/                    # gRPC protobuf definitions (language-agnostic)
├── docker/                   # Dockerfiles
└── docker-compose.yml        # Local dev environment
```

---

## Architecture Conventions

### Backend

- **Default DB**: MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres` and updating `database.host` / `database.database`.
- **Config philosophy**: `default.yaml` is production-safe (TLS on, auth on, DB encrypted). `local.yaml` (gitignored) is required for local development and explicitly opts out of these protections. `server.environment` (`local` | `development` | `staging` | `production`) is the single source of truth for environment classification. Security checks use this field, not URL heuristics. `AppConfig::validate()` panics on hardcoded passwords, missing TLS certs, and HTTP issuer URLs outside localhost. Secrets: `database.password_file` supports Docker/K8s secret mounts; weak password rejection outside local.
- **Migrations**: refinery (supports both PostgreSQL and MSSQL). Embedded in server binary. `server migrate` subcommand runs migrations standalone. `database.run_migrations_on_startup` controls whether migrations run on serve (default `false` in prod config, `true` in local).
- **Error handling**: SNAFU per-layer enums. `repo::Error` → `svc::Error` → `api::UsersError`.
- **API responses**: Success = raw JSON (no envelope), HTTP 2xx. Error = JSON with `type`/`title`/`status`/`detail` fields (RFC 9457 Problem Details subset), HTTP 4xx/5xx.
- **Security headers**: Backend adds `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`, `Permissions-Policy`, and HSTS (when TLS on). Frontend Next.js config adds CSP and matching headers.
- **Request limits**: `DefaultBodyLimit::max()` enforces `server.max_request_body_size` (default 1 MiB). `TimeoutLayer` handles request timeouts.
- **Rate limiting**: `tower_governor` on `/api/*` when `rate_limit.enabled: true`. Production default is `enabled: true` (10 req/s, burst 20); local dev should override to `false` in `local.yaml`.
- **Auth**: OIDC Bearer token. JWT validated via JWKS (jsonwebtoken crate, manual discovery or manual endpoints). Algorithm restricted to `auth.allowed_algorithms` allowlist. Supports `auth.require_email_verified` and `auth.clock_skew_seconds`. JIT user provisioning with email domain whitelist and role resolution via `ProvisioningPolicy`. **Auth is enabled by default**; local development requires `local.yaml` to explicitly disable it.
- **Authorization**: IdP-driven RBAC. The IdP is the authority for role assignment. On each request with a valid token, the middleware syncs the user's `role`, `display_name`, and `email_verified` from the current OIDC claims. Role is derived from claims via `ProvisioningPolicy::resolve_role()`, which maps well-known role names (admin/administrator/superuser → admin, manager/supervisor → manager) from the configured claim source (`roles` or `groups`). Hierarchical: Admin > Manager > User. Routes: list/get/update require Manager+; create/delete require Admin. When auth is disabled, all requests pass through.
- **Repository pattern**: Service depends on `UserRepo` trait. Both `MssqlUserRepo` (tiberius) and `PostgresUserRepo` (sqlx) are implemented under `repo/src/user_repo/`. Adapter-specific testcontainers tests live in the same file as the adapter (`#[cfg(test)]`). A `test-helpers` feature provides `MockUserRepo` for upstream unit tests.
- **gRPC**: Port 50051. Service-to-service only. Provides `health.v1.HealthService` with `HealthCheck` for k8s probes. Production requires `grpc.tls.enabled: true` (mTLS when `ca_cert_path` is set). `grpc.auth_enabled` adds real JWT validation via sync JWKS cache with background refresh.
- **Audit**: `AuditExporter` trait (Strategy pattern) with `NoopExporter` default. `AuditService` uses a bounded async channel for backpressure (drops events when full with explicit metrics). Export retry: 3x exponential backoff. PII redaction via `audit.pii_mode: redact` masks email and sub fields. Exporters: `SyslogExporter` (UDP/TCP/TCP+TLS), `OtelLogsExporter` (OTLP HTTP JSON).
- **Optimistic Locking**: `users` table has a `version` column. `UPDATE` increments `version` and checks `WHERE id = ? AND version = ?`, returning `409 CONFLICT` on stale data.
- **Tracing**: `#[tracing::instrument]` on service functions. Request ID via `x-request-id` header.

### Frontend

- **Auth**: next-auth with generic OIDC. JWT session strategy. Auto-redirect to IdP.
- **API calls**: Browser-side axios points at `/api/proxy`; the Next.js BFF proxy reads the encrypted next-auth JWT cookie server-side and attaches the Bearer token before forwarding to the backend. No access token is exposed to client JavaScript.
- **Validation**: Zod schemas in `schemas/`. Single source for validation + types.
- **API types**: Single authority — TypeScript types are derived from the backend OpenAPI spec. Run `pnpm openapi:gen` (frontend) after `cd backend && cargo run -p server -- gen-openapi > ../docs/openapi.json` to regenerate `schema.d.ts` when DTOs change.
- **State**: Zustand (UI state), SWR (server cache).
- **Routing**: Next.js App Router. `(auth)` = public route group (no URL impact), `dashboard/` = protected route segment.
- **Styling**: Tailwind CSS v4 (CSS-first config).
- **CSP**: Production uses nonce-based CSP via `proxy.ts`. Nonce is propagated to Server Components via copied request headers. Local dev skips strict CSP.

---

## Code Style

### Backend (Rust)

Backend Rust code style follows the local **`/rust-best-practices`** skill (Apollo GraphQL handbook). Use it when writing, reviewing, or refactoring Rust — covers ownership, error handling (`Result`/`?`), performance, clippy lints, testing, generics, and type-state patterns.

#### Import Style

- Run `cargo fmt --all` (or `mise run check:be`) before committing — `reorder_imports = true` is enabled in `backend/rustfmt.toml`.
- **Do not insert blank lines between `use` statements** in the same file. Stable rustfmt treats blank lines as group boundaries and only sorts within each group, so a misplaced blank line will leave imports partially unsorted. Keep all `use` lines contiguous at the top of the file.
- Use short names in code after importing at the top. Fully-qualified inline paths are acceptable only for disambiguation or single-use items.

### Frontend (TypeScript / React / Next.js)

Frontend code style follows the local **`/vercel-react-best-practices`** skill (Vercel Labs). Use it when writing, reviewing, or refactoring frontend code — covers Server Components, data fetching, re-render optimization, bundle size, Suspense boundaries, and caching.

---

## Code Quality

**CRITICAL**: Before reporting task completion, always run:

```bash
mise run check
```

This runs the full backend and frontend verification flow via `mise`: Rust/TypeScript format checks, linting, spell check, compile/build validation, and automated tests.

`lefthook` `pre-commit` is intentionally lighter: format/lint/spell-check only. There is no `pre-push` hook by default; full verification belongs in CI and in the final agent validation step above.

---

## Documentation Discipline

- Before reporting task completion, check the [Change-to-Documentation Matrix](#change-to-documentation-matrix).
- If a change modifies public API shape, regenerate and commit generated API artifacts instead of editing them by hand.
- If a change introduces or renames a domain concept, seam, layer, or invariant, update `CONTEXT.md`.
- If a change changes how developers run the project, update `README.md`.
- If a change changes how agents should implement, validate, or review future work, update `AGENTS.md`.
- If a change alters the repeatable vertical-slice feature flow, update `docs/how-to-add-feature.md`.
- If a change records a durable architecture decision with meaningful alternatives, add an ADR under `docs/adr/`.

---

## Sensitive Data Policy

- Never log passwords, tokens, or PII.
- Secrets go through environment variables, never in `config/default.yaml`.
- `.env` and `config/local.yaml` are in `.gitignore`.

---

## Commits

Conventional commits: `feat:`, `fix:`, `chore:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`. Enforced by `committed` via lefthook `commit-msg` hook.

---

## Local Agent Skills

The following skills live under `.agents/skills/` and are available via slash-command or auto-trigger. Invoke them explicitly when the situation matches.

### Code Style

- **`/rust-best-practices`** — Rust ownership, error handling, performance, clippy, testing, generics, type-state. Use when writing, reviewing, or refactoring backend code.
- **`/vercel-react-best-practices`** — React/Next.js Server Components, data fetching, re-render optimization, bundle size, Suspense, caching. Use when writing, reviewing, or refactoring frontend code.

### Productivity

- **`/grill-me`** — Interview the user relentlessly about a plan or design until reaching shared understanding, resolving each branch of the decision tree. Use when user wants to stress-test a plan, get grilled on their design, or mentions "grill me".

### Engineering

- **`/diagnose`** — Hard bugs, performance regressions, "something is broken/failing". Follows reproduce → minimise → hypothesise → instrument → fix → regression-test. Use **instead of ad-hoc debugging**.
- **`/grill-with-docs`** — Before a big refactor or plan, challenge it against `CONTEXT.md` and `docs/adr/` domain model. Use when change touches architecture or vocabulary.
- **`/improve-codebase-architecture`** — When the codebase feels shallow, seams are wrong, or a bug revealed missing testability. Uses `CONTEXT.md` + `docs/adr/` as authority.
- **`/tdd`** — Building a feature or fixing a bug test-first. Red-green-refactor, one vertical slice at a time.
- **`/zoom-out`** — Broader context on an unfamiliar code section. Use when you (the agent) are lost in a module and need high-level orientation.
