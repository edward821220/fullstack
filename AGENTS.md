# AGENTS.md — Fullstack Template

## Directory Structure

```
project-root/
├── backend/                  # Rust Cargo Workspace
│   ├── Cargo.toml            # Workspace root, shared dependencies
│   ├── config/
│   │   ├── default.yaml      # Default config (checked in)
│   │   └── local.example.yaml # Local override (gitignored)
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

## Architecture Conventions

### Backend

- **Default DB**: MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres` and updating `database.host` / `database.database`.
- **Config philosophy**: `default.yaml` is production-safe (TLS on, auth on, DB encrypted). `local.yaml` (gitignored) is required for local development and explicitly opts out of these protections. `AppConfig::validate()` panics on hardcoded passwords, missing TLS certs, and HTTP issuer URLs outside localhost.
- **Migrations**: refinery (supports both PostgreSQL and MSSQL). Embedded in server binary, run on startup.
- **Error handling**: SNAFU per-layer enums. `repo::Error` → `svc::Error` → `api::UsersError`.
- **API responses**: Success = raw JSON (no envelope), HTTP 2xx. Error = JSON with `type`/`title`/`status`/`detail` fields (RFC 9457 Problem Details subset), HTTP 4xx/5xx.
- **Auth**: OIDC Bearer token. JWT validated via JWKS (jsonwebtoken crate, manual discovery or manual endpoints). JIT user provisioning with email domain whitelist and role resolution via `ProvisioningPolicy`. **Auth is enabled by default**; local development requires `local.yaml` to explicitly disable it.
- **Authorization**: IdP-driven RBAC. The IdP is the authority for role assignment. On each request with a valid token, the middleware syncs the user's `role`, `display_name`, and `email_verified` from the current OIDC claims. Role is derived from claims via `ProvisioningPolicy::resolve_role()`, which maps well-known role names (admin/administrator/superuser → admin, manager/supervisor → manager) from the configured claim source (`roles` or `groups`). Hierarchical: Admin > Manager > User. Routes: list/get/update require Manager+; create/delete require Admin. When auth is disabled, all requests pass through.
- **Repository pattern**: Service depends on `UserRepo` trait. Both `MssqlUserRepo` (tiberius) and `PostgresUserRepo` (sqlx) are implemented under `repo/src/user_repo/`. Adapter-specific testcontainers tests live in the same file as the adapter (`#[cfg(test)]`). A `test-helpers` feature provides `MockUserRepo` for upstream unit tests.
- **gRPC**: Port 50051. Service-to-service only. Provides `health.v1.HealthService` with `HealthCheck` for k8s probes. Project-specific gRPC contracts should be added per service.
- **Audit**: `AuditExporter` trait (Strategy pattern) with `StdoutExporter` default. `AuditService` uses an async channel to avoid blocking handlers. Events are structured JSON with `request_id`, `trace_id`, `client_ip`, `user_agent`. Future exporters: syslog, OTLP logs.
- **Optimistic Locking**: `users` table has a `version` column. `UPDATE` increments `version` and checks `WHERE id = ? AND version = ?`, returning `409 CONFLICT` on stale data.
- **Tracing**: `#[tracing::instrument]` on service functions. Request ID via `x-request-id` header.

### Frontend

- **Auth**: next-auth with generic OIDC. JWT session strategy. Auto-redirect to IdP.
- **API calls**: axios interceptor auto-attaches Bearer token from next-auth session.
- **Validation**: Zod schemas in `schemas/`. Single source for validation + types.
- **API types**: Single authority — TypeScript types are derived from the backend OpenAPI spec. Run `pnpm openapi:gen` (frontend) after `cd backend && cargo run -p server -- gen-openapi > ../docs/openapi.json` to regenerate `schema.d.ts` when DTOs change.
- **State**: Zustand (UI state), SWR (server cache).
- **Routing**: Next.js App Router. `(auth)` = public route group (no URL impact), `dashboard/` = protected route segment.
- **Styling**: Tailwind CSS v4 (CSS-first config).

## Code Style

Backend Rust code style follows the local **`/rust-best-practices`** skill (Apollo GraphQL handbook). Use it when writing, reviewing, or refactoring Rust — covers ownership, error handling (`Result`/`?`), performance, clippy lints, testing, generics, and type-state patterns.

Frontend TypeScript / React / Next.js code style follows the local **`/vercel-react-best-practices`** skill (Vercel Labs). Use it when writing, reviewing, or refactoring frontend code — covers Server Components, data fetching, re-render optimization, bundle size, Suspense boundaries, and caching.

## Code Quality

**CRITICAL**: Before reporting task completion, always run:

```bash
mise run check:be check:fe
```

This runs the full backend and frontend verification flow via `mise`: Rust/TypeScript format checks, linting, spell check, compile/build validation, and automated tests.

`lefthook` `pre-commit` is intentionally lighter: format/lint/spell-check only. There is no `pre-push` hook by default; full verification belongs in CI and in the final agent validation step above.

## Documentation Discipline

- When modifying architecture, file structure, or domain vocabulary, **always sync `CONTEXT.md`** (or create it if missing). `CONTEXT.md` is the authority for domain language and seam names.
- When modifying build steps, tooling, or project conventions, **always sync `AGENTS.md`** and `README.md` so future agents do not re-discover the same information.
- Before reporting task completion, verify that any file/structure changes mentioned in `AGENTS.md` or `README.md` are still accurate.

## Sensitive Data Policy

- Never log passwords, tokens, or PII.
- Secrets go through environment variables, never in `config/default.yaml`.
- `.env` and `config/local.yaml` are in `.gitignore`.

## Commits

Conventional commits: `feat:`, `fix:`, `chore:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`. Enforced by `committed` via lefthook `commit-msg` hook.

## Local Agent Skills

The following skills live under `.agents/skills/` and are available via slash-command or auto-trigger. Invoke them explicitly when the situation matches.

### Code Style

- **`/rust-best-practices`** — Rust ownership, error handling, performance, clippy, testing, generics, type-state. Use when writing, reviewing, or refactoring backend code.
- **`/vercel-react-best-practices`** — React/Next.js Server Components, data fetching, re-render optimization, bundle size, Suspense, caching. Use when writing, reviewing, or refactoring frontend code.

### Productivity

- **`/grill-me`** — Interview the user relentlessly about a plan or design until reaching shared understanding, resolving each branch of the decision tree. Use when the user wants to stress-test a plan, get grilled on their design, or mentions "grill me".

### Engineering

- **`/diagnose`** — Hard bugs, performance regressions, "something is broken/failing". Follows reproduce → minimise → hypothesise → instrument → fix → regression-test. Use **instead of ad-hoc debugging**.
- **`/grill-with-docs`** — Before a big refactor or plan, challenge it against `CONTEXT.md` and `docs/adr/` domain model. Use when the change touches architecture or vocabulary.
- **`/improve-codebase-architecture`** — When the codebase feels shallow, seams are wrong, or a bug revealed missing testability. Uses `CONTEXT.md` + `docs/adr/` as authority.
- **`/tdd`** — Building a feature or fixing a bug test-first. Red-green-refactor, one vertical slice at a time.
- **`/zoom-out`** — Broader context on an unfamiliar code section. Use when you (the agent) are lost in a module and need high-level orientation.


