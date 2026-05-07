// Runtime-validation seam: only schemas that need transforms live here.
// For everything else, import directly from @/lib/api/gen/zod.gen.ts or
// @/lib/api/gen/types.gen.ts.

import { zPaginatedUserResponse, zUpdateUserRequest, zUserResponse } from "@/lib/api/gen/zod.gen";

/** Partial variant for update forms. */
export const updateUserSchema = zUpdateUserRequest.partial();

/** Transforms int64 (bigint) → number to match frontend expectations. */
export const userResponseSchema = zUserResponse.transform((u) => ({
  ...u,
  version: Number(u.version),
}));

/** Transforms int64 (bigint) → number to match frontend expectations. */
export const paginatedUserResponseSchema = zPaginatedUserResponse.transform((data) => ({
  ...data,
  page: Number(data.page),
  per_page: Number(data.per_page),
  total: Number(data.total),
  data: data.data.map((u) => ({
    ...u,
    version: Number(u.version),
  })),
}));
