# Fullstack Template Readiness Review

_Date: 2026-05-04_

## TL;DR

**結論：目前還不能算是「完整可交付給銀行地端專案直接開工」的 fullstack template。**

現在的狀態比較準確地說是：

- **Backend**：有明確分層、可工作的 authenticated skeleton、MSSQL/Postgres 抽象、JIT provisioning、基本 RBAC、OpenAPI、gRPC 啟動骨架。
- **Frontend**：有最小可用的 Next.js + next-auth shell，並且 API type 已經能從 backend OpenAPI 生成。
- **整體定位**：適合拿來做 **accelerator / spike starter**，但還不夠資格當 **銀行地端專案的標準起始模板**。

如果要作為未來多個銀行案子的正式 template，我建議把目前狀態定義為：

- **Template readiness**: `Partial`
- **Current maturity**: `Authenticated skeleton`
- **Recommended label**: `v0 template candidate`, not `production-grade template`

## 我這次 review 的判準

我用的是這個 repo 現有約定與你的目標，而不是抽象 checklist：

- `AGENTS.md` 定義的 backend/frontend 分層與 template 規範
- 你的目標：**銀行地端**、**SSO 對接**、**metrics / gRPC 預先開好**、**新團隊進場時每層責任清楚**
- 我沒有把「沒有 production docker-compose」當主 blocker，因為這個 repo 的 compose 目前本來就更像 **local dev only**；真正的缺口是 **部署基線與運維文件缺失**

## 現在已經做得不錯的地方

### 1. Backend 分層已經有 template 雛形

這點其實是目前最好的部分。

- `backend/crates/config/src/lib.rs`
- `backend/crates/dto/src/lib.rs`
- `backend/crates/model/src/*.rs`
- `backend/crates/repo/src/lib.rs`
- `backend/crates/svc/src/lib.rs`
- `backend/crates/server/src/*`

優點：

- `config` / `dto` / `model` / `repo` / `svc` / `server` 的邊界是清楚的
- `svc` 依賴 `UserRepo` trait，不直接綁 DB impl
- `repo` 同時提供 MSSQL / Postgres 實作，對 template 來說很合理
- migration 在啟動時跑，對本地開發與新專案 bootstrap 很方便

### 2. Auth 主流程是通的

Backend：

- `backend/crates/server/src/middleware/oidc.rs`
- `backend/crates/server/src/middleware/authz.rs`
- `backend/crates/server/src/handlers/users.rs`
- `backend/crates/svc/src/lib.rs`

Frontend：

- `frontend/src/lib/auth/config.ts`
- `frontend/src/app/api/auth/[...nextauth]/route.ts`
- `frontend/src/app/dashboard/layout.tsx`
- `frontend/src/app/(auth)/login/page.tsx`

目前已具備：

- JWT Bearer token 驗證
- JWKS cache
- discovery/manual backend 模式
- JIT provisioning
- route-level RBAC
- frontend next-auth OIDC login
- dashboard route protection

對一般公司內部系統 skeleton 來說，這其實已經超過「只有 hello world」的程度。

### 3. API contract 單一來源方向是對的

- `backend/crates/server/src/openapi.rs`
- `frontend/src/lib/api/schema.d.ts`
- `frontend/src/lib/api/types.ts`
- `frontend/package.json` (`openapi:gen`)

這個方向是對的，因為 template 最怕 frontend/backend type drift。

### 4. Tooling 也算整齊

- `mise.toml`
- backend / frontend checks 已經分開
- repo 內有基本測試

這讓 template 比較容易複製到新專案。

## 為什麼我認為「還不能算完整 template」

下面是我認為真正會卡銀行地端專案起步的缺口。

---

## P0 - Blocker

**這些不補，我不會把它叫做完整 fullstack template。**

### P0-1. Observability 只做到 logging/tracing，沒有 metrics / OTLP baseline

證據：

- `backend/crates/server/src/main.rs` 只有 `init_tracing()` 與 `TraceLayer`
- repo 雖然有 `tracing-opentelemetry` / `opentelemetry-otlp` 依賴，但沒有 exporter wiring
- 沒有 `/metrics` endpoint
- README 也直接寫了 `OTLP and production hardening are planned but not yet implemented`

為什麼這是 blocker：

- 銀行地端專案不是只要能跑，還要能 **被維運**
- template 必須預先定義：request metrics、DB latency、gRPC latency、error rate、trace correlation、service naming
- 如果第一個專案團隊還要自己從零決定 metrics/tracing 標準，這個 template 就沒有完成它的工作

必做：

- 加上 Prometheus metrics 或等價 metrics exporter
- 加上可開關的 OTLP tracing/export baseline
- 把 HTTP / DB / gRPC 共同 tags/attributes 定義好
- 定義 request id / trace id / user subject 的 log correlation 規則

### P0-2. gRPC 目前只是 placeholder，不是可複用的 service template

證據：

- `proto/greetings/v1/greetings_service.proto`
- `backend/crates/server/src/main.rs`
- `backend/crates/grpc/src/main.rs`

目前只有：

- `SayHello`
- `HealthCheck`

問題：

- 沒有示範 **domain-oriented proto layout**
- 沒有 gRPC auth/interceptor pattern
- 沒有 gRPC error mapping / metadata / request tracing / deadline handling / reflection strategy
- 沒有示範 gRPC 如何與 `svc` 層對接，而不是直接停在 demo 層

如果你要把它給未來團隊當起點，現在這組 gRPC 比較像「證明 tonic 有接起來」，還不是「可以照這個方式複製新服務」。

### P0-3. Frontend 還只是 authenticated shell，不是可落地的 feature template

證據：

- `frontend/src/app/page.tsx` 只 redirect 到 `/dashboard`
- `frontend/src/app/dashboard/page.tsx` 只是 welcome 頁
- `frontend/src/hooks/useUsers.ts` 有 hook，但沒有完整 feature flow
- `frontend/src/stores/app.ts` 只有 sidebar state
- `frontend/src/components/features/providers.tsx` 只有 `SessionProvider`

缺什麼：

- 一個完整的 feature slice 範例（list / detail / create / update）
- loading / empty / error / retry 標準模式
- form + schema + mutation + toast/error handling pattern
- App Router 下 server component / client component 邊界示範
- server-side API adapter pattern
- frontend RBAC/claim-driven UI gating baseline

現在的新團隊拿這個 frontend 開專案，還是得自己定義「功能模組到底該怎麼長」。這表示 template 還沒把 frontend 這層做完整。

### P0-4. 銀行 SSO 支援在 frontend / backend 不對稱

證據：

- Backend: `backend/crates/server/src/middleware/oidc.rs` 支援 `discovery` 與 `manual` 模式
- Frontend: `frontend/src/lib/auth/config.ts` 永遠使用 `issuer` + `wellKnown`

這代表：

- backend 已經考慮企業 IdP 可能要 manual JWKS/discovery override
- frontend 還是假設 IdP 能走標準 `.well-known/openid-configuration`

這在銀行地端場景是很現實的問題。很多企業 IdP/OIDC 閘道雖然「差不多」OIDC，但前端整合細節往往不夠標準、憑證鏈也可能特殊。

如果 backend 可以 manual，但 frontend 不行，template 的 SSO story 就是不完整的。

### P0-5. 缺少「新專案接手文件」與本地覆蓋配置範本

證據：

- `backend/config/local.example.yaml` 不存在
- README 有 quick start，但沒有真正的 onboarding / extension guide
- 沒有說明每一層「新增一個 feature」的標準做法
- 沒有 bank IdP claim mapping / role mapping 的操作說明

這會直接影響 template 目的：

> 讓未來開始專案的人知道怎麼分層

現在 repo 有架構雛形，但**還沒有把做法文件化到可移交程度**。

---

## P1 - High

**這些不是立刻阻塞 repo 跑起來，但會在第一個真正專案中很快出事。**

### P1-1. 首次以既有 email 綁定 identity 的路徑沒有同步 OIDC attributes

證據：

- `backend/crates/svc/src/lib.rs:186-236`

問題細節：

- 若使用者已存在於 `users` table，但尚未建立 `user_identity`
- `provision_user()` 會：
  - 先 `find_by_email()`
  - 命中 `Some(u)` 後直接使用既有 `u`
  - 建立 `identity`
  - **但不會同步 `display_name` / `role` / `email_verified`**

影響：

- 第一次 SSO 進來時，角色與姓名可能仍是 DB 舊值
- 這對「先建帳、後綁 SSO」場景是實際 bug

### P1-2. Frontend auth lifecycle 不完整

證據：

- `frontend/src/lib/auth/config.ts`
- `frontend/src/lib/auth/types.ts`

目前有：

- access token 放入 session

但缺：

- refresh token rotation strategy
- access token expiry / refresh failure handling
- session error surfaced to UI
- 登出後 redirect / post-logout flow 策略

對企業 SSO 專案來說，這些通常不是 optional。

### P1-3. OpenAPI / contract 文檔還不夠完整

證據：

- `backend/crates/server/src/openapi.rs`
- `backend/crates/server/src/handlers/health.rs`
- `backend/crates/server/src/handlers/users.rs`

問題：

- `/health/ready` 沒進 OpenAPI
- error responses 沒完整標註在 path annotations
- auth-related failure contract 沒被文件化

template 不只要 API 能用，還要把 contract 說清楚。

### P1-4. Backend 沒有真正的 metrics / audit / security event baseline

這點跟 P0 observability 不同。

P0 是「沒有可觀測性骨架」；P1 是「沒有銀行案常見的審計/安全事件骨架」。

目前只有一般 tracing，沒有：

- audit event model
- security-sensitive action logging contract
- auth success/failure / role denial event taxonomy

如果未來每個專案都各自亂長，template 價值會很低。

### P1-5. 測試覆蓋不夠 template 級

Backend：

- `backend/crates/server/tests/integration_test.rs` 主要是 health 與 authz middleware 測試
- 沒有真正跑 OIDC middleware + provisioning + users routes 的整體整合測試

Frontend：

- `frontend/__tests__/features.test.ts` 主要測 type/schema/config 結構
- `frontend/__tests__/example.test.ts` 還是 placeholder
- 沒有 login redirect flow / protected layout / API hook / error state 測試

如果 template 要被複製很多次，測試應該是「保護模板本身」，不是只保護單一示範 endpoint。

### P1-6. App Router 的 server-side API pattern 缺位

證據：

- `frontend/src/lib/api/client.ts` 的註解已經寫明：這是 client-side only
- 但 repo 內沒有對應的 server-side adapter

這會讓新團隊在 Next.js App Router 下自己決定：

- server component 怎麼打 backend
- route handler 怎麼取 session token
- SSR / RSC / client fetch 要怎麼分層

對 template 來說，這個缺口不小。

---

## P2 - Medium

### P2-1. Frontend 缺少真正的「feature module 標準寫法」

目前 `components/features` 只有 `providers.tsx`，其實還沒形成 feature architecture 的教學價值。

建議至少補一個完整垂直 slice，例如 `users`：

- `features/users/components/*`
- `features/users/api/*`
- `features/users/hooks/*`
- `features/users/schema/*`
- `features/users/lib/*`

### P2-2. README 誠實，但也等於承認目前不是完整 template

- `README.md` 已經明寫：frontend 是 minimal authenticated shell、gRPC 是 placeholder、OTLP 未實作

這很好，因為沒有亂吹；但也代表目前 repo 自己已經在說它還不是完整模板。

### P2-3. 缺少對外部整合 extension points 的示範

銀行專案常會很快碰到：

- object storage
- batch / scheduler
- message broker
- internal service-to-service gRPC client
- third-party core banking adapter

template 不一定要把全部接好，但至少應該有一個明確 extension story。

### P2-4. 沒有 role/claims mapping 文件化策略

程式裡有：

- `role_claim_source: roles | groups`
- `ProvisioningPolicy::resolve_role()` 的 well-known role mapping

但沒有把「若銀行 IdP claim 名稱不同、群組命名不同、要怎麼改」寫成 onboarding 文檔。

---

## P3 - Low

### P3-1. 有一些明顯 placeholder 痕跡

- `frontend/__tests__/example.test.ts`
- generic dashboard welcome 頁
- generic greetings proto/service

這些不影響架構，但會降低 template 的完成感。

### P3-2. UI baseline 太薄

雖然不需要把 template 做成產品，但至少應該有：

- app shell
- sidebar/header pattern
- empty/error/loading skeleton
- form page / list page / detail page 樣板

不然 frontend team 還是得從零定義 UI 結構。

---

## Ready / Not Ready 判定

## Backend

### Ready

- crate layering
- DB abstraction (MSSQL / Postgres)
- migration bootstrap
- basic OIDC JWT validation
- JIT provisioning 基本主流程
- RBAC middleware 與 Problem Details 基本模式
- OpenAPI generation

### Not ready

- metrics / OTLP baseline
- gRPC production pattern
- audit/security events baseline
- first-login-by-email attribute sync correctness
- auth / API contract documentation completeness
- template-level integration tests

## Frontend

### Ready

- Next.js App Router shell
- next-auth OIDC baseline
- protected layout
- API type generation from OpenAPI
- basic SWR / Zod / Zustand presence

### Not ready

- complete feature slice example
- server-side API adapter
- robust auth lifecycle handling
- enterprise/bank IdP flexibility
- RBAC-driven UI conventions
- real integration/component tests

---

## 建議的 Todo List

## Phase 0 - 把它升級成真正的 template

- [ ] **補 observability baseline**
  - HTTP metrics
  - DB query metrics
  - gRPC metrics
  - OTLP tracing exporter
  - `/metrics` or equivalent scrape endpoint
  - log/trace correlation fields 文檔

- [ ] **把 gRPC 從 demo 變成 template**
  - 新增 domain-style proto example
  - 定義 interceptor / auth / metadata / deadline / error mapping pattern
  - 補 gRPC service 如何呼叫 `svc` 層的範例
  - 規範 reflection 在 dev/prod 的開關

- [ ] **補 frontend 完整 feature slice 範例**
  - users list page
  - detail page
  - create/update form
  - loading/empty/error/retry pattern
  - client/server component 分界範例

- [ ] **補銀行 SSO 對接完整故事**
  - frontend 支援非標準 IdP 的設定策略
  - token refresh / expiry strategy
  - claim mapping / role mapping 文檔
  - self-signed / custom CA 的前後端說明

- [ ] **補 onboarding 文件**
  - `backend/config/local.example.yaml`
  - `docs/architecture.md` 或 ADR
  - `docs/how-to-add-feature.md`
  - `docs/how-to-integrate-bank-sso.md`
  - `docs/ops-observability.md`

## Phase 1 - 修正實際會踩雷的 correctness / contract 問題

- [x] **修正 `provision_user()` 既有 email 首次綁定 identity 不同步屬性的問題**
- [x] **補 `/health/ready` 與 error responses 的 OpenAPI 文件**
- [x] **補 backend OIDC + users route 整合測試**
- [ ] **補 frontend login/protected route/API hook 測試**
- [x] **新增 server-side backend API adapter for Next App Router**

## Phase 2 - 提升 template 可複制度

- [x] **建立 frontend feature module 目錄規範並附範例**（維持目前 `components/features/` + `hooks/` + `lib/api/` 架構，以 users 作為完整垂直範例）
- [ ] **建立 backend 新增 endpoint / service / repo / dto 的 checklist**
- [x] **提供一個 reference business domain slice，而不是只有 users/demo**（users 已為完整 slice；accounts 孤兒檔案已移除）
- [x] **加入 audit event / security event abstraction**
- [ ] **~~補部署基線文件（不是 docker-compose prod，而是 bank on-prem 的部署要求與 override points）~~**（維持 local docker-compose / Dockerfile 即可）

## Phase 3 - 收尾與體驗提升

- [ ] 刪掉 placeholder 測試與 demo 文案
- [x] 補 app shell / common layout / empty/error/loading UI
- [ ] 補範例環境命名、secret naming、config naming conventions

---

## 我會怎麼定義「完成版 template」

如果你要把這個 repo 當成未來銀行專案的正式起點，我認為至少要滿足下面這個標準：

### 必須具備

- **Backend**
  - 明確 crate layering
  - 至少一個完整 vertical slice
  - OIDC + RBAC + provisioning + audit baseline
  - metrics + tracing + health + readiness
  - REST + gRPC 的標準實作樣板
  - OpenAPI / proto / errors / tests 完整對齊

- **Frontend**
  - auth shell 以外，至少一個完整 feature page flow
  - client/server data fetching pattern 都有
  - API error handling standard
  - role-aware UI baseline
  - testing baseline

- **Docs / Ops**
  - local/dev/on-prem deployment guidance
  - config examples
  - feature onboarding guide
  - bank SSO integration guide
  - observability guide

### 到那時我才會說

> 這是一個可被複製到新銀行案子的 fullstack template。

---

## 最後結論

**現在的 repo：可以當作不錯的「起始骨架」，但還不能當作「完整 fullstack template」。**

如果你要我用一句最準的話來描述目前狀態：

> **backend 已經有 template 架子，frontend 仍偏 skeleton，ops/observability/gRPC/template docs 還沒補齊。**

所以我的建議是：

- 短期不要把它宣稱成 completed template
- 先標成 `template candidate`
- 依照上面的 `P0 -> P1 -> P2` 補齊

等 P0/P1 做完，這個 repo 才會真的進入「可作為銀行地端專案起點」的範圍。
