# Fullstack Template

Fullstack monorepo template for banking-adjacent services. Backend: Rust (Axum + tonic). Frontend: TypeScript (Next.js + Tailwind).

> **Status**: Authenticated skeleton with OIDC + MSSQL/Postgres support. RBAC enforced on user endpoints. OTLP and production hardening are planned but not yet implemented.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend API | Rust, Axum |
| Backend gRPC | Rust, tonic |
| Database (default) | MS SQL Server |
| Database (alternative) | PostgreSQL (sqlx) |
| Migrations | refinery (MS SQL + PostgreSQL) |
| Error Handling | SNAFU |
| Observability | tracing (OTLP planned) |
| Auth | Generic OIDC (Dex, Keycloak, Azure AD, Google, Bank SSO) |
| Authorization | Hierarchical RBAC enforced on user endpoints (Admin > Manager > User). Admin: create/delete; Manager+: list/get/update. Auth-disabled mode passes through. Fine-grained ACLs should be added per project. |
| Frontend | TypeScript, Next.js (App Router) — minimal authenticated shell |
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

## Quick Start

### 1. Start Database

```bash
# MS SQL Server (requires --profile mssql)
docker compose --profile mssql up -d

# Or PostgreSQL (default, no profile needed)
docker compose up -d
```

### 2. Run Backend & Frontend Locally

```bash
# Backend (http://localhost:3001, gRPC :50051)
mise run dev:be

# Frontend — first run needs pnpm install
pnpm -C frontend install
mise run dev:fe
```

### 3. (Optional) Start Local OIDC Services

```bash
docker compose --profile full up -d
# Starts Dex + MS SQL Server for local auth testing
# Dex is pre-configured with test users (admin/manager/user)
```

### 4. Frontend Environment

```bash
cp frontend/.env.example frontend/.env.local
# Edit .env.local with your OIDC provider settings
```

### 5. Run Checks

```bash
mise run check:be check:fe
```

`pre-commit` runs a lighter `lefthook` gate: formatter, linter, and spell check only. Full compile/build/test verification stays in CI and in the final local check above.

## Endpoints

| URL | Description |
|-----|-------------|
| `http://localhost:3000` | Frontend |
| `http://localhost:3001/api/v1/users` | REST API |
| `http://localhost:3001/docs` | API docs (Scalar) |
| `http://localhost:3001/health` | Liveness |
| `http://localhost:3001/health/ready` | Readiness (DB check) |
| `grpc://localhost:50051` | gRPC |

## Configuration

### Backend (`backend/config/default.yaml`)

Override via `config/local.yaml` (gitignored) or `APP_*` env vars:

```bash
APP_SERVER__PORT=3002
APP_DATABASE__DATABASE_URL="postgres://user:pass@localhost:5432/db"
```

Default database is MS SQL Server. Switch to PostgreSQL by setting `database.driver: postgres` and updating `database.database_url` to a `postgres://` connection string.

### Auth Setup

Enable auth: set `auth.enabled: true` in `backend/config/default.yaml` (disabled by default for local development).

#### Local OIDC with Dex

1. Start Dex only: `docker compose --profile oidc up -d`
   Or start Dex + MS SQL Server together: `docker compose --profile full up -d`
2. Copy `frontend/.env.example` to `frontend/.env.local` (pre-configured for Dex)
3. Enable auth in backend config: `APP_AUTH__ENABLED=true`
4. Test users: `admin`/`Admin123!`, `manager`/`Manager123!`, `user`/`User123!`

Dex config and client credentials are pre-defined in `docker/dex/config.yaml`. No manual setup needed.

OIDC provider via frontend env vars (see `frontend/.env.example`):

| Variable | Description |
|----------|-------------|
| `AUTH_OIDC_ID` | OIDC client ID |
| `AUTH_OIDC_SECRET` | OIDC client secret |
| `AUTH_OIDC_ISSUER` | OIDC issuer URL |
| `NEXTAUTH_URL` | Public frontend URL |
| `NEXTAUTH_SECRET` | JWT encryption secret (`openssl rand -hex 32`) |

For bank on-prem IdPs with self-signed certificates:
```yaml
# backend/config/default.yaml
auth:
  discovery_mode: "manual"
  manual_endpoints:
    jwks_uri: "https://idp.bank.com/keys"
    issuer: "https://idp.bank.com"
  danger_accept_invalid_certs: true
```

## Architecture

See [AGENTS.md](./AGENTS.md) for detailed architecture and development conventions.

## Scope & Limitations

This template provides an authenticated skeleton. The following areas are intentionally minimal:

- **Frontend**: Minimal authenticated shell (login → dashboard → sign out). Business UI patterns (loading/empty/error states, data tables, forms) should be added per project.
- **gRPC**: Placeholder `SayHello`/`HealthCheck` endpoints. Demonstrates tonic integration pattern but not a production service.
- **Authorization**: Hierarchical role-based guard (Admin > Manager > User) is provided. Fine-grained permissions, resource-level ACLs, or ABAC should be implemented per project.
- **OIDC**: Backend supports both discovery and manual endpoint modes (for enterprise IdPs with self-signed certs). Frontend uses standard OIDC discovery only — for non-standard IdPs, customize `frontend/src/lib/auth/config.ts`.
- **Observability**: Tracing with request IDs is wired. OTLP export is planned but not implemented.
