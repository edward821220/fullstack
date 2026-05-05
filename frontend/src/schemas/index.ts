// Runtime-validation seam: only schemas that need transforms live here.
// For everything else, import directly from @/lib/api/gen/zod.gen.ts or
// @/lib/api/gen/types.gen.ts.

import { zPaginatedUserResponse, zUpdateUserRequest } from "@/lib/api/gen/zod.gen";

/** Partial variant for update forms. */
export const updateUserSchema = zUpdateUserRequest.partial();

/** Transforms int64 (bigint) → number to match frontend expectations. */
export const paginatedUserResponseSchema = zPaginatedUserResponse.transform((data) => ({
  data: data.data,
  page: Number(data.page),
  per_page: Number(data.per_page),
  total: Number(data.total),
}));
