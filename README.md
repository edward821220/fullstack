# Fullstack Template

Fullstack monorepo template for banking-adjacent services. Backend: Rust (Axum + tonic). Frontend: TypeScript (Next.js + Tailwind).

> **Status**: Template candidate with OIDC, REST, gRPC, Prometheus metrics, OTLP export, and a reusable protected frontend slice.

## Who This Is For

This README is the human developer entry point. Keep it focused on setup, local operation, important endpoints, and commands developers need on day one.

For implementation rules, architecture conventions, code style, quality gates, and documentation-update discipline, use [AGENTS.md](./AGENTS.md).

## Documentation Map

| File | Audience | Purpose |
|------|----------|---------|
| [README.md](./README.md) | Developers | Local setup, run commands, endpoints, configuration entry points |
| [AGENTS.md](./AGENTS.md) | AI agents | Architecture rules, coding conventions, quality gates, doc-update rules |
| [CONTEXT.md](./CONTEXT.md) | AI agents + developers | Domain vocabulary, seam names, architectural language |
| [docs/how-to-add-feature.md](./docs/how-to-add-feature.md) | AI agents + developers | Step-by-step vertical-slice feature workflow |
| `docs/adr/` | AI agents + developers | Accepted architectural decisions and trade-offs |

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend API | Rust, Axum |
| Backend gRPC | Rust, tonic |
| Database (default) | MS SQL Server |
| Database (alternative) | PostgreSQL (sqlx) |
| Migrations | refinery |
| Auth | Generic OIDC (Dex, Keycloak, Azure AD, Bank SSO) |
| Authorization | Hierarchical RBAC (Admin > Manager > User) |
| Frontend | TypeScript, Next.js (App Router) |
| Styling | Tailwind CSS v4, shadcn/ui |
| Validation | Zod |
| State | Zustand + SWR |
| API Docs | Scalar (utoipa) |

## Prerequisites

Install [mise](https://mise.jdx.dev), then:

```bash
mise install   # Rust, Node, pnpm, lefthook, typos, committed
mise trust
lefthook install
```

Install frontend dependencies once:

```bash
pnpm -C frontend install
```

## Quick Start

`docker/dev/docker-compose.yml` is for local development only.

### 1. Create Local Config

```bash
cp backend/config/local.example.yaml backend/config/local.yaml
cp frontend/.env.example frontend/.env.local
```

### 2. Start Database

```bash
# MS SQL Server (default)
docker compose -f docker/dev/docker-compose.yml --profile mssql-dex up -d

# Or PostgreSQL
docker compose -f docker/dev/docker-compose.yml --profile postgres-dex up -d
```

### 3. Run Backend & Frontend

```bash
# Backend (`server` binary; REST on http://localhost:3001)
mise run dev:be

# Frontend
mise run dev:fe
```

`mise run dev:be` starts the combined `server` binary. REST is always served; gRPC is only exposed when `grpc.enabled: true` in `backend/config/local.yaml`.

### 4. (Optional) Enable Auth

Local dev defaults to `auth.enabled: false`. To enable:

```bash
# Edit backend/config/local.yaml: set auth.enabled: true
# Dex is already included in mssql-dex and postgres-dex profiles
# Frontend .env.local is already pre-configured for Dex
```

### 5. Run Checks

```bash
mise run check
# Or separately:
# mise run check:be
# mise run check:fe
```

`pre-commit` runs a lighter `lefthook` gate: formatter, linter, and spell check only. Full compile/build/test verification stays in CI and in the final local check above.

## Common Commands

| Command | Purpose |
|---------|---------|
| `mise run dev:be` | Run backend server |
| `mise run dev:fe` | Run frontend dev server |
| `mise run check` | Run full backend + frontend verification |
| `mise run check:be` | Run backend format, lint, check, tests |
| `mise run check:fe` | Run frontend format, lint, typecheck, build, tests |
| `mise run openapi:gen` | Regenerate OpenAPI JSON, TypeScript types, and Zod schemas |
| `cargo run --bin grpc-server` | Run the standalone gRPC server from `backend/` |
| `docker buildx bake --print` | Preview Docker build configuration |
| `docker buildx bake --load --set *.platforms=linux/amd64 backend` | Build backend image locally |
| `docker buildx bake --load --set *.platforms=linux/amd64 frontend` | Build frontend image locally |

## Endpoints

| URL | Description |
|-----|-------------|
| `http://localhost:3000` | Frontend |
| `http://localhost:3001/api/v1/users` | REST API |
| `http://localhost:3001/docs` | API docs (Scalar, only when `server.docs_enabled: true`) |
| `http://localhost:3001/health` | Liveness |
| `http://localhost:3001/health/ready` | Readiness (DB check) |
| `http://localhost:3001/metrics` | Prometheus metrics (only when `observability.metrics_enabled: true`) |
| `grpc://localhost:50051` | gRPC (only when `grpc.enabled: true` or when running `grpc-server`) |

## Configuration

Backend config lives in `backend/config/default.yaml` (production-safe defaults). Override via `backend/config/local.yaml` (gitignored) or `APP_*` env vars:

```bash
APP_SERVER__PORT=3002
APP_SERVER__ENVIRONMENT=production
APP_OBSERVABILITY__OTLP__ENABLED=true
```

By default, the server looks for `config/default.yaml` and `config/local.yaml` relative to the working directory. Override the config directory with `--config-dir <path>` or `APP_CONFIG_DIR` env var (CLI flag takes precedence). This is useful for K8s deployments where config is mounted at a different path:

```bash
# Via CLI flag
server --config-dir /etc/app/config

# Via environment variable
APP_CONFIG_DIR=/etc/app/config server
```

`server.environment` (`local` | `development` | `staging` | `production`) controls security strictness: production enforces all checks, local disables most for developer convenience.

Default database is MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres`.

gRPC is disabled by default in both `backend/config/default.yaml` and `backend/config/local.example.yaml`. Enable it by setting `grpc.enabled: true` in your local `backend/config/local.yaml`, or run the standalone gRPC binary from `backend/` with `cargo run --bin grpc-server`.

Frontend local environment lives in `frontend/.env.local`, copied from `frontend/.env.example`.

### Auth

Auth is **enabled by default** in production config. For local development, `local.example.yaml` disables it.

To connect a real IdP, set these in `frontend/.env.local`:

| Variable | Description |
|----------|-------------|
| `ENVIRONMENT` | `local` / `development` / `staging` / `production` — controls CSP and auth validation strictness |
| `AUTH_OIDC_ID` | OIDC client ID |
| `AUTH_OIDC_SECRET` | OIDC client secret |
| `AUTH_OIDC_ISSUER` | OIDC issuer URL |
| `NEXTAUTH_SECRET` | JWT encryption secret (`openssl rand -hex 32`) |

For bank on-prem IdPs with private-CA certificates, mount the CA bundle and use `SSL_CERT_FILE`. Do **not** disable TLS verification in production.

## Docker Build

Multi-platform images are defined in `docker/docker-bake.hcl`. CI builds `linux/amd64,linux/arm64` and pushes to GCP Artifact Registry.

```bash
# Verify configuration
docker buildx bake -f docker/docker-bake.hcl --print

# Local smoke test (single platform, no push)
docker buildx bake -f docker/docker-bake.hcl --load --set *.platforms=linux/amd64 backend
docker buildx bake -f docker/docker-bake.hcl --load --set *.platforms=linux/amd64 frontend

# CI: build + push (requires env vars: GCP_PROJECT_ID, AR_REGION, AR_REPO, TAG)
docker buildx bake -f docker/docker-bake.hcl --push
```

See [AGENTS.md](./AGENTS.md) for Docker build conventions, registry variable details, and CI prerequisites.

## Development Workflow

Most implementation work is expected to be delegated to AI agents. Developers should:

- **Start from README**: use this file to set up and run the project.
- **Send coding tasks to agents**: agents must follow [AGENTS.md](./AGENTS.md).
- **Use the feature guide**: new domain features should follow [docs/how-to-add-feature.md](./docs/how-to-add-feature.md).
- **Regenerate generated API artifacts**: run `mise run openapi:gen` after backend DTO or `utoipa` route changes.
- **Run the full gate**: run `mise run check` before handing work off or merging.

## Scope & Limitations

This template provides an authenticated skeleton. The following areas are intentionally minimal:

- **Frontend**: One complete protected dashboard slice is included, but domain-specific forms, workflows, and authorization-aware navigation should still be added per project.
- **gRPC**: `health.v1.HealthService` is included for k8s probes. Project-specific protobuf contracts should be added per service.
- **Authorization**: Hierarchical role-based guard (Admin > Manager > User) is provided. Fine-grained permissions, resource-level ACLs, or ABAC should be implemented per project.
- **Observability**: Prometheus metrics and OTLP trace export are wired, but collector topology, dashboards, alerts, and retention remain project-level work.
