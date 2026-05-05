import { z } from "zod/v4";

// Re-export auto-generated Zod schemas from OpenAPI spec.
// Run `pnpm openapi:gen` (or `mise run openapi:gen`) after backend DTO changes.
import { schemas as generated } from "@/lib/api/schema.zod";

// ── Zod Schemas (generated from OpenAPI) ────────────────────────────────────

export const userResponseSchema = generated.UserResponse;
export const paginatedUserResponseSchema = generated.PaginatedUserResponse;
export const createUserSchema = generated.CreateUserRequest;
export const updateUserSchema = generated.UpdateUserRequest.partial();

// ── Inferred Types ───────────────────────────────────────────────────────────

export type UserResponse = z.infer<typeof userResponseSchema>;
export type PaginatedUserResponse = z.infer<typeof paginatedUserResponseSchema>;
export type CreateUserInput = z.infer<typeof createUserSchema>;
export type UpdateUserInput = z.infer<typeof updateUserSchema>;
