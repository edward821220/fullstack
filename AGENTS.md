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
│       ├── repo/             # Repository pattern. Trait + PostgresUserRepo impl.
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
├── proto/                    # gRPC protobuf definitions (language-agnostic)
├── docker/                   # Dockerfiles
├── k8s/helm/                 # Helm chart
└── docker-compose.yml        # Local dev environment
```

## Architecture Conventions

### Backend

- **Default DB**: MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres` and updating `database.database_url` to a `postgres://` connection string.
- **Migrations**: refinery (supports both PostgreSQL and MSSQL). Embedded in server binary, run on startup.
- **Error handling**: SNAFU per-layer enums. `repo::Error` → `svc::Error` → `api::UsersError`.
- **API responses**: Success = raw JSON (no envelope), HTTP 2xx. Error = JSON with `type`/`title`/`status`/`detail` fields (RFC 9457 Problem Details subset), HTTP 4xx/5xx.
- **Auth**: OIDC Bearer token. JWT validated via JWKS (jsonwebtoken crate, manual discovery or manual endpoints). JIT user provisioning with email domain whitelist and role resolution via `ProvisioningPolicy`.
- **Authorization**: IdP-driven RBAC. The IdP is the authority for role assignment. On each request with a valid token, the middleware syncs the user's `role`, `display_name`, and `email_verified` from the current OIDC claims. Role is derived from claims via `ProvisioningPolicy::resolve_role()`, which maps well-known role names (admin/administrator/superuser → admin, manager/supervisor → manager) from the configured claim source (`roles` or `groups`). Hierarchical: Admin > Manager > User. Routes: list/get/update require Manager+; create/delete require Admin. When auth is disabled, all requests pass through.
- **Repository pattern**: Service depends on `UserRepo` trait. Both `MssqlUserRepo` (tiberius) and `PostgresUserRepo` (sqlx) are implemented.
- **gRPC**: Port 50051. Service-to-service only. Currently provides `SayHello` and `HealthCheck` as placeholder patterns.
- **Tracing**: `#[tracing::instrument]` on service functions. Request ID via `x-request-id` header.

### Frontend

- **Auth**: next-auth with generic OIDC. JWT session strategy. Auto-redirect to IdP.
- **API calls**: axios interceptor auto-attaches Bearer token from next-auth session.
- **Validation**: Zod schemas in `schemas/`. Single source for validation + types.
- **API types**: Single authority — TypeScript types are derived from the backend OpenAPI spec. Run `pnpm openapi:gen` (frontend) after `cargo run -p server -- gen-openapi > openapi.json` (backend) to regenerate `schema.d.ts` when DTOs change.
- **State**: Zustand (UI state), SWR (server cache).
- **Routing**: Next.js App Router. `(auth)` = public route group (no URL impact), `dashboard/` = protected route segment.
- **Styling**: Tailwind CSS v4 (CSS-first config).

## Rust Code Style

### Borrowing & Ownership
- Prefer `&T` over `.clone()` unless ownership is required.
- Use `&str` over `String`, `&[T]` over `Vec<T>` in function parameters.
- Small `Copy` types (≤24 bytes) may be passed by value.

### Error Handling
- Return `Result<T, E>` — never `unwrap()`/`expect()` outside tests.
- Use `?` operator. No manual match chains for error propagation.
- Use `thiserror` for libs, `anyhow` for binaries only.
- Do not silently swallow errors with `if let Err(_) =`.

### Performance
- Avoid cloning in loops. Use `.iter()` instead of `.into_iter()` for `Copy` types.
- Prefer iterators over manual loops. Avoid intermediate `.collect()`.
- Run `cargo clippy -- -D clippy::perf` for performance hints.

### Linting
Always run: `cargo clippy --all-targets --all-features -- -D warnings`

Key lints: `redundant_clone`, `large_enum_variant`, `needless_collect`.
Use `#[expect(clippy::lint)]` (not `#[allow]`) with justification comment.

### Testing
- Test naming: `process_should_return_error_when_input_empty()`.
- One assertion per test.
- Doc tests (`///`) for public API examples.

### Imports
Group: `std` → external crates → `crate` → `super`/`self`.
Separate groups with blank lines.

### Comments
- No comments on self-documenting code. Add comments only for non-obvious business logic.
- `//` = why (safety, workarounds, design rationale).
- `///` = what + how (public API docs).
- Every TODO needs an issue number: `// TODO(#42): ...`

### Generics & Dispatch
- Prefer generics (static dispatch). Use `dyn Trait` only for heterogeneous collections.
- Box at API boundaries, not internally.

## TypeScript Code Style

### Imports
- Use `import type` for type-only imports.
- Group: React/Next → external libs → `@/` aliases → relative.

### Components
- Prefer Server Components. Add `"use client"` only when needed (hooks, events).
- Extract expensive work into memoized components (`React.memo`).
- Extract static JSX outside the component body.
- Use ternary (`a ? <B /> : null`) for conditional rendering, not `&&`.

### Data Fetching
- Parallelize independent fetches with `Promise.all()`. No waterfalls.
- Use SWR for client-side fetch dedup and caching.
- Defer await into branches where actually consumed.
- Start promises early, await late in API routes.

### Re-render Optimization
- Use functional `setState(prev => ...)` for stable callbacks.
- Pass function to `useState()` for expensive initial values (`useState(() => heavy())`).
- Use `useTransition()` / `startTransition()` for non-urgent updates.
- Subscribe to derived booleans, not raw state values.

### Bundle Size
- Use `next/dynamic` with `ssr: false` for heavy client-only components.
- Import directly from module paths — avoid barrel file re-exports.
- Defer third-party analytics/logging loads until after hydration.

### Rendering
- Use `content-visibility: auto` for long lists.
- Reduce SVG coordinate precision (≤2 decimal places).
- Group DOM CSS changes via class toggling, not individual style writes.

## Code Quality

**CRITICAL**: Before reporting task completion, always run:

```bash
mise run check
```

This runs: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo check`, `cargo test`, `pnpm format:check`, `eslint`, `tsc --noEmit`, `pnpm build`, `vitest`.

## Sensitive Data Policy

- Never log passwords, tokens, or PII.
- Secrets go through environment variables, never in `config/default.yaml`.
- `.env` and `config/local.yaml` are in `.gitignore`.

## Commits

Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `ci:`, `perf:`.
