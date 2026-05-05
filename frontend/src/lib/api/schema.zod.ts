// @ts-nocheck
// Auto-generated from OpenAPI spec by openapi-zod-client.
// Uses zod v3 APIs; skipped from type-check because project runs zod v4.
// Runtime is compatible; do not edit manually. Regenerate with `mise run openapi:gen`.
import { makeApi, Zodios, type ZodiosOptions } from "@zodios/core";
import { z } from "zod";

const UserResponse = z
  .object({
    created_at: z.string(),
    display_name: z.string(),
    email: z.string(),
    email_verified: z.boolean(),
    id: z.string().uuid(),
    role: z.string(),
    updated_at: z.string(),
  })
  .passthrough();
const PaginatedUserResponse = z
  .object({
    data: z.array(UserResponse),
    page: z.number().int().gte(0),
    per_page: z.number().int().gte(0),
    total: z.number().int().gte(0),
  })
  .passthrough();
const ErrorResponse = z
  .object({
    detail: z.string(),
    status: z.number().int().gte(0),
    title: z.string(),
    type: z.string(),
  })
  .passthrough();
const CreateUserRequest = z
  .object({
    display_name: z.string().min(1).max(100),
    email: z.string().min(1).max(100).email(),
  })
  .passthrough();
const UpdateUserRequest = z
  .object({ display_name: z.union([z.string(), z.null()]) })
  .partial()
  .passthrough();
const HealthResponse = z.object({ status: z.string(), version: z.string() }).passthrough();

export const schemas = {
  UserResponse,
  PaginatedUserResponse,
  ErrorResponse,
  CreateUserRequest,
  UpdateUserRequest,
  HealthResponse,
};

const endpoints = makeApi([
  {
    method: "get",
    path: "/api/v1/users",
    alias: "list_users",
    requestFormat: "json",
    parameters: [
      {
        name: "page",
        type: "Query",
        schema: z.number().int().gte(0).optional(),
      },
      {
        name: "per_page",
        type: "Query",
        schema: z.number().int().gte(0).optional(),
      },
    ],
    response: PaginatedUserResponse,
    errors: [
      {
        status: 400,
        description: `Invalid pagination parameters`,
        schema: ErrorResponse,
      },
      {
        status: 401,
        description: `Missing or invalid bearer token`,
        schema: ErrorResponse,
      },
      {
        status: 403,
        description: `Insufficient role (requires manager)`,
        schema: ErrorResponse,
      },
      {
        status: 500,
        description: `Internal server error`,
        schema: ErrorResponse,
      },
    ],
  },
  {
    method: "post",
    path: "/api/v1/users",
    alias: "create_user",
    requestFormat: "json",
    parameters: [
      {
        name: "body",
        type: "Body",
        schema: CreateUserRequest,
      },
    ],
    response: UserResponse,
    errors: [
      {
        status: 400,
        description: `Invalid input`,
        schema: ErrorResponse,
      },
      {
        status: 401,
        description: `Missing or invalid bearer token`,
        schema: ErrorResponse,
      },
      {
        status: 403,
        description: `Insufficient role (requires admin)`,
        schema: ErrorResponse,
      },
      {
        status: 500,
        description: `Internal server error`,
        schema: ErrorResponse,
      },
    ],
  },
  {
    method: "get",
    path: "/api/v1/users/:id",
    alias: "get_user",
    requestFormat: "json",
    parameters: [
      {
        name: "id",
        type: "Path",
        schema: z.string().uuid(),
      },
    ],
    response: UserResponse,
    errors: [
      {
        status: 401,
        description: `Missing or invalid bearer token`,
        schema: ErrorResponse,
      },
      {
        status: 403,
        description: `Insufficient role (requires manager)`,
        schema: ErrorResponse,
      },
      {
        status: 404,
        description: `User not found`,
        schema: ErrorResponse,
      },
      {
        status: 500,
        description: `Internal server error`,
        schema: ErrorResponse,
      },
    ],
  },
  {
    method: "put",
    path: "/api/v1/users/:id",
    alias: "update_user",
    requestFormat: "json",
    parameters: [
      {
        name: "body",
        type: "Body",
        schema: UpdateUserRequest,
      },
      {
        name: "id",
        type: "Path",
        schema: z.string().uuid(),
      },
    ],
    response: UserResponse,
    errors: [
      {
        status: 401,
        description: `Missing or invalid bearer token`,
        schema: ErrorResponse,
      },
      {
        status: 403,
        description: `Insufficient role (requires manager)`,
        schema: ErrorResponse,
      },
      {
        status: 404,
        description: `User not found`,
        schema: ErrorResponse,
      },
      {
        status: 500,
        description: `Internal server error`,
        schema: ErrorResponse,
      },
    ],
  },
  {
    method: "delete",
    path: "/api/v1/users/:id",
    alias: "delete_user",
    requestFormat: "json",
    parameters: [
      {
        name: "id",
        type: "Path",
        schema: z.string().uuid(),
      },
    ],
    response: z.void(),
    errors: [
      {
        status: 401,
        description: `Missing or invalid bearer token`,
        schema: ErrorResponse,
      },
      {
        status: 403,
        description: `Insufficient role (requires admin)`,
        schema: ErrorResponse,
      },
      {
        status: 404,
        description: `User not found`,
        schema: ErrorResponse,
      },
      {
        status: 500,
        description: `Internal server error`,
        schema: ErrorResponse,
      },
    ],
  },
  {
    method: "get",
    path: "/health",
    alias: "health",
    requestFormat: "json",
    response: HealthResponse,
  },
  {
    method: "get",
    path: "/health/ready",
    alias: "health_ready",
    requestFormat: "json",
    response: HealthResponse,
    errors: [
      {
        status: 503,
        description: `Database health check failed`,
        schema: ErrorResponse,
      },
    ],
  },
]);

export const api = new Zodios(endpoints);

export function createApiClient(baseUrl: string, options?: ZodiosOptions) {
  return new Zodios(baseUrl, endpoints, options);
}
