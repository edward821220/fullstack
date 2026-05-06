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

`docker-compose.yml` is for local development only.

### 1. Create Local Config

```bash
cp backend/config/local.example.yaml backend/config/local.yaml
cp frontend/.env.example frontend/.env.local
```

### 2. Start Database

```bash
# MS SQL Server (default)
docker compose --profile mssql up -d

# Or PostgreSQL
docker compose --profile postgres up -d
```

### 3. Run Backend & Frontend

```bash
# Backend (http://localhost:3001, gRPC :50051)
mise run dev:be

# Frontend
mise run dev:fe
```

### 4. (Optional) Enable Auth Locally

```bash
# Start Dex (pre-configured with test users: admin/manager/user)
docker compose --profile full up -d

# Enable auth in backend
APP_AUTH__ENABLED=true mise run dev:be

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

## Endpoints

| URL | Description |
|-----|-------------|
| `http://localhost:3000` | Frontend |
| `http://localhost:3001/api/v1/users` | REST API |
| `http://localhost:3001/docs` | API docs (Scalar) |
| `http://localhost:3001/health` | Liveness |
| `http://localhost:3001/health/ready` | Readiness (DB check) |
| `http://localhost:3001/metrics` | Prometheus metrics |
| `grpc://localhost:50051` | gRPC |

## Configuration

Backend config lives in `backend/config/default.yaml` (production-safe defaults). Override via `config/local.yaml` (gitignored) or `APP_*` env vars:

```bash
APP_SERVER__PORT=3002
APP_OBSERVABILITY__OTLP__ENABLED=true
```

Default database is MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres`.

Frontend local environment lives in `frontend/.env.local`, copied from `frontend/.env.example`.

### Auth

Auth is **enabled by default** in production config. For local development, `local.example.yaml` disables it.

To connect a real IdP, set these in `frontend/.env.local`:

| Variable | Description |
|----------|-------------|
| `AUTH_OIDC_ID` | OIDC client ID |
| `AUTH_OIDC_SECRET` | OIDC client secret |
| `AUTH_OIDC_ISSUER` | OIDC issuer URL |
| `NEXTAUTH_SECRET` | JWT encryption secret (`openssl rand -hex 32`) |

For bank on-prem IdPs with private-CA certificates, mount the CA bundle and use `SSL_CERT_FILE`. Do **not** disable TLS verification in production.

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
