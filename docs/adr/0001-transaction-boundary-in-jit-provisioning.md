# ADR：JIT 使用者佈建中的交易邊界

## 狀態

**開放** — 已識別為關鍵風險，待設計決策。

## 背景

`UserService::provision_user` 方法實作了 Just-In-Time (JIT) OIDC 使用者佈建。當使用者首次透過 OIDC 登入時，系統必須：

1. 驗證使用者的電子郵件網域是否符合允許清單。
2. 檢查 OIDC 身分是否已存在 (`find_by_identity`)。
3. 若身分已存在，同步 OIDC 屬性（name、role、email_verified）並返回。
4. 若身分不存在，檢查本地使用者是否依電子郵件存在 (`find_by_email`)。
5. 若使用者存在，同步 OIDC 屬性。
6. 若使用者不存在，建立新的本地使用者 (`create`)。
7. 建立 OIDC 身分記錄，將本地使用者連結到 IdP (`create_identity`)。

### 目前實作

目前在 `backend/crates/svc/src/user_service.rs` 的實作將這些步驟作為一連串獨立的 `UserRepo` 方法呼叫執行。此多步驟流程周圍**沒有交易邊界**。

### 競爭條件

在 `find_by_email` 和 `create` 之間，並發請求可能建立具有相同電子郵件的使用者。Postgres 和 MSSQL 配接器都在 `create()` 中有 `find_by_email` 防護，但該防護本身是非原子性的 SELECT-then-INSERT 模式，因此競爭條件是真實存在且未經測試的。

### 部分失敗情境

最嚴重的風險是步驟 6（使用者建立/同步）與步驟 7（身分建立）之間的**部分失敗**：

```
sync_oidc_attributes(user.id, ...)  ->  OK
create_identity(user.id, provider, issuer, sub)  ->  FAIL (network, DB, etc.)
```

失敗後：
- 資料庫中存在本地 `User` 記錄。
- **沒有** `UserIdentity` 記錄將使用者連結到 OIDC 提供者。
- 下次登入嘗試將依電子郵件找到使用者，再次同步屬性，並嘗試再次建立身分。
- 系統是**最終一致性**的，但非原子性。

### 為何這對銀行本地部署至關重要

在銀行環境中，部分使用者建立引發合規與稽核疑慮：

- **稽核追蹤缺口**：稽核員可能看到沒有對應 `UserIdentity` 的 `User` 記錄，無法將使用者追蹤回 IdP。
- **角色指派完整性**：若 `sync_oidc_attributes` 成功但 `create_identity` 失敗，使用者的角色已依 OIDC 聲明更新，但連結到 IdP 的連結缺失。這破壞了 IdP 驅動的 RBAC 不變性。
- **等冪性假設**：下游系統（例如 SIEM、存取審查）可能假設每個本地使用者都有可驗證的外部身分。

## 決策

**延後** — 修復此問題需要在 `UserRepo` 縫合處中新增跨資料庫交易抽象，這是重大的介面變更。我們將在交易埠設計完成後處理此問題。

## 後果

### 若不修復

- JIT 佈建維持最終一致性。
- 部分失敗可在下次登入時復原，但會留下暫時無效狀態。
- 銀行稽核員可能標記身分建立缺乏原子性的問題。

### 若修復

- 需要對 `UserRepo` 新增交易埠（例如 `begin_transaction()`、`commit()`、`rollback()`）。
- `PostgresUserRepo` 和 `MssqlUserRepo` 都必須使用驅動程式特定的交易 API 實作該埠（Postgres 使用 `sqlx::Transaction`，MSSQL 使用 `tiberius` 交易）。
- `provision_user` 必須接受交易上下文，並在其中執行所有操作。
- 測試必須涵蓋復原路徑（例如，身分建立失敗觸發使用者建立復原）。

## 相關檔案

- `backend/crates/svc/src/user_service.rs` — `provision_user` 實作
- `backend/crates/repo/src/user_repo/mod.rs` — `UserRepo` trait（無交易方法）
- `backend/crates/repo/src/user_repo/postgres.rs` — Postgres 配接器
- `backend/crates/repo/src/user_repo/mssql.rs` — MSSQL 配接器

## 備註

- `MockUserRepo`（記憶體內偽造）需要簡單的交易實作以用於上游單元測試。
- 兩個配接器中的 `create` 方法已經執行 `find_by_email` + `INSERT`。若引進交易，方法內的防護可能變得多餘（交易隔離等級處理唯一性）。
