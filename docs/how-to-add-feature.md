# How to Add a Feature

This guide walks through adding a new domain feature (e.g. `orders`, `transactions`) to the fullstack template. Follow the same layering pattern used by the existing `users` feature.

---

## Table of Contents

1. [Backend — Add a Vertical Slice](#backend--add-a-vertical-slice)
2. [Frontend — Add a Feature Module](#frontend--add-a-feature-module)
3. [OpenAPI Type Regeneration](#openapi-type-regeneration)
4. [Quick Checklist](#quick-checklist)
5. [Documentation Updates](#documentation-updates)

---

## Backend — Add a Vertical Slice

The backend is a Cargo workspace with six domain layers plus a shared infrastructure crate (`infra`). A new feature touches the six domain layers; `infra` only changes when you add a new cross-cutting concern (e.g. a new audit exporter or health checker).

### 1. Model (`backend/crates/model/src/`)

Define the domain entity and any related structs.

```rust
// backend/crates/model/src/order.rs
use time::OffsetDateTime;
use uuid::Uuid;

pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

Register it in `model/src/lib.rs`:

```rust
pub mod order;
pub mod user;
pub mod user_identity;
```

### 2. DTO (`backend/crates/dto/src/lib.rs`)

Add request/response types with `serde` and `utoipa::ToSchema`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateOrderRequest {
    pub user_id: Uuid,
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedOrderResponse {
    pub data: Vec<OrderResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
```

### 3. Repository (`backend/crates/repo/src/`)

Create `order_repo/` following the same pattern as `user_repo/`.

```
repo/src/order_repo/
  mod.rs        — module exports
  trait.rs      — trait definition
  any.rs        — optional type-erased wrapper if the feature needs multi-adapter bootstrap
  mssql.rs      — MSSQL adapter
  postgres.rs   — PostgreSQL adapter
  test_helpers.rs — MockOrderRepo (behind #[cfg(feature = "test-helpers")])
```

Trait example:

```rust
#[async_trait]
pub trait OrderRepo: Send + Sync + Clone {
    type Tx: Transaction;

    async fn begin_transaction(&self) -> Result<Self::Tx>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Order>>;
    async fn create(&self, user_id: Uuid, amount: i64, currency: &str) -> Result<Order>;
    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<Order>, u64)>;
}
```

Export in `repo/src/lib.rs`:

```rust
pub mod order_repo;
pub use order_repo::{MssqlOrderRepo, PostgresOrderRepo, OrderRepo};
// If the feature needs runtime adapter erasure at bootstrap boundaries:
// pub use order_repo::AnyOrderRepo;
```

### 4. Service (`backend/crates/svc/src/`)

Create `order_service.rs` and follow the same generic pattern as `UserService<R: UserRepo>`.

```rust
pub struct OrderService<R: OrderRepo> {
    repo: R,
}

#[async_trait]
pub trait OrderServiceTrait<R: OrderRepo>: Send + Sync {
    async fn get_order(&self, id: Uuid) -> Result<Order>;
    async fn create_order(&self, user_id: Uuid, amount: i64, currency: &str) -> Result<Order>;
    async fn list_orders(&self, page: u64, per_page: u64) -> Result<(Vec<Order>, u64)>;
}
```

Register in `svc/src/lib.rs`:

```rust
pub mod order_service;
pub use order_service::{OrderService, OrderServiceTrait};
```

### 5. Handler (`backend/crates/server/src/handlers/`)

Create `orders.rs`:

```rust
pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/orders", get(list_orders).post(create_order))
        .route("/orders/{id}", get(get_order))
        .with_state(state)
}
```

Register in `server/src/handlers/mod.rs`:

```rust
pub mod health;
pub mod orders;
pub mod users;
```

Wire into `server/src/rest_server.rs`:

```rust
let api_routes = Router::new()
    .merge(users::routes(app_state.clone()))
    .merge(orders::routes(app_state.clone()))
    .route_layer(axum_middleware::from_fn_with_state(...));
```

### 6. OpenAPI Registration

Add the new handler functions to `server/src/openapi.rs`:

```rust
#[openapi(
    paths(
        handlers::users::list_users,
        handlers::orders::list_orders,
        handlers::orders::get_order,
        handlers::orders::create_order,
    ),
    components(schemas(
        dto::OrderResponse,
        dto::CreateOrderRequest,
        dto::PaginatedOrderResponse,
    )),
)]
pub struct ApiDoc;
```

---

## Frontend — Add a Feature Module

Use the `users` feature as a reference. A new feature usually touches:

### 1. API Layer (`frontend/src/lib/api/<feature>/`)

```ts
// frontend/src/lib/api/orders/client.ts
import * as api from "@/lib/api/client";
import { orderResponseSchema, paginatedOrderResponseSchema } from "@/schemas";
import type {
  CreateOrderRequest,
  OrderResponse,
  PaginatedOrderResponse,
} from "@/lib/api/gen/types.gen";

export async function getOrdersPage(page = 1, perPage = 20) {
  return api.get<PaginatedOrderResponse>(
    `/orders?page=${page}&per_page=${perPage}`,
    paginatedOrderResponseSchema,
  );
}

export async function createOrder(input: CreateOrderRequest) {
  return api.post<OrderResponse>("/orders", input, orderResponseSchema);
}
```

If the feature also needs Server Component access, add `frontend/src/lib/api/orders/server.ts` beside it, following `users/server.ts`.

### 2. Schema (`frontend/src/schemas/`)

**No need to hand-write Zod schemas.** Frontend Zod schemas are auto-generated from the OpenAPI spec.

If you need frontend runtime validation for a new feature, first add utoipa validation annotations to the backend DTO:

```rust
// backend/crates/dto/src/lib.rs
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateOrderRequest {
    pub user_id: Uuid,
    #[schema(minimum = 1)]
    pub amount: i64,
    #[schema(min_length = 3, max_length = 3, example = "USD")]
    pub currency: String,
}
```

Then run `mise run openapi:gen`, and the frontend will automatically get the corresponding generated artifacts:

```ts
// frontend/src/lib/api/gen/zod.gen.ts (auto-generated)
export const zCreateOrderRequest = z.object({
  user_id: z.uuid(),
  amount: z.int().gte(1),
  currency: z.string().min(3).max(3),
});
```

Only add code to `frontend/src/schemas/index.ts` when you need runtime transforms or derived schemas that the generator should not own:

```ts
// frontend/src/schemas/index.ts
import { zCreateOrderRequest } from "@/lib/api/gen/zod.gen";
import { z } from "zod/v4";

export const createOrderSchema = zCreateOrderRequest.extend({
  currency: zCreateOrderRequest.shape.currency.transform((value) => value.toUpperCase()),
});
export type CreateOrderInput = z.infer<typeof createOrderSchema>;
```

### 3. Hook (`frontend/src/hooks/`)

```ts
// frontend/src/hooks/useOrders.ts
import useSWR from "swr";
import { getOrdersPage } from "@/lib/api/orders/client";

export function useOrders(page = 1, perPage = 20) {
  return useSWR(`/orders?page=${page}&per_page=${perPage}`, () => getOrdersPage(page, perPage));
}
```

### 4. Components (`frontend/src/components/features/orders/`)

```
components/features/orders/
  orders-table.tsx
  order-form.tsx
```

### 5. Pages (`frontend/src/app/dashboard/orders/`)

```
app/dashboard/orders/
  page.tsx           — list view
  new/page.tsx       — create form
  [id]/page.tsx      — detail view
  [id]/edit/page.tsx — edit form
```

---

## OpenAPI Type Regeneration

Whenever backend DTOs change, regenerate frontend TypeScript types from the OpenAPI spec.

### One-shot command (backend → frontend)

```bash
mise run openapi:gen
```

This runs two sub-tasks in sequence:

1. **`mise run openapi:gen:be`** — generates `docs/openapi.json` from the Rust OpenAPI doc
2. **`mise run openapi:gen:fe`** — converts `docs/openapi.json` into `frontend/src/lib/api/gen/types.gen.ts` and `frontend/src/lib/api/gen/zod.gen.ts`

### Manual steps

If you need to run them separately:

```bash
# 1. Generate OpenAPI JSON from backend
cd backend
cargo run -p server -- gen-openapi > ../docs/openapi.json

# 2. Generate TypeScript types + Zod schemas from OpenAPI JSON
cd frontend
pnpm openapi:gen
```

The generation output is configured in `frontend/openapi-ts.config.ts`:

```ts
export default defineConfig({
  input: "../docs/openapi.json",
  output: "src/lib/api/gen",
  plugins: ["@hey-api/typescript", "zod"],
});
```

> **Rule of thumb**: always run `mise run openapi:gen` after modifying any `#[derive(ToSchema)]` struct or `#[utoipa::path]` handler in the backend.

---

## Quick Checklist

### Backend

- [ ] Model struct in `model/src/<feature>.rs`
- [ ] DTOs in `dto/src/lib.rs` with `ToSchema`
- [ ] Repo trait + adapters in `repo/src/<feature>_repo/` (and `any.rs` if you need type erasure at bootstrap boundaries)
- [ ] Service trait + impl in `svc/src/<feature>_service.rs` using the same generic seam pattern as `UserService<R>`
- [ ] Handler routes in `server/src/handlers/<feature>.rs`
- [ ] Register handler in `handlers/mod.rs`
- [ ] Wire routes in `rest_server.rs`
- [ ] Register schemas/paths in `openapi.rs`
- [ ] Unit tests in each crate
- [ ] Integration tests in `server/tests/<feature>.rs`

### Frontend

- [ ] Generated Zod schema available via `lib/api/gen/zod.gen.ts`
- [ ] Optional runtime transform or derived schema in `schemas/index.ts`
- [ ] API functions in `lib/api/<feature>/client.ts` and optional `lib/api/<feature>/server.ts`
- [ ] SWR hook in `hooks/use<Feature>.ts`
- [ ] Components in `components/features/<feature>/`
- [ ] Pages in `app/dashboard/<feature>/`
- [ ] Tests under `frontend/__tests__/` for the new API, hooks, schemas, or routes
- [ ] Run `mise run openapi:gen` to sync types

### Cross-Cutting

- [ ] Run `mise run check:be` and `mise run check:fe`
- [ ] Run `mise run openapi:gen` after DTO or `utoipa` route changes
- [ ] Update `CONTEXT.md` if the feature introduces new domain vocabulary, seams, or invariants
- [ ] Update `README.md` if developers need new setup, env vars, ports, endpoints, or commands
- [ ] Update `AGENTS.md` if future agents need a new implementation rule or quality gate
- [ ] Update this doc if the vertical-slice pattern changes

---

## Documentation Updates

Use [AGENTS.md](../AGENTS.md) as the authority for documentation ownership. For normal feature work:

- **Domain language**: update `CONTEXT.md` when adding new concepts or renaming seams.
- **Developer operation**: update `README.md` only when humans need new setup/run/config information.
- **Agent rules**: update `AGENTS.md` only when the implementation pattern or validation rule changes.
- **Generated API contract**: never hand-edit generated artifacts; run `mise run openapi:gen`.
