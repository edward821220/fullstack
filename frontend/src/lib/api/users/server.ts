import * as api from "@/lib/api/server";
import { paginatedUserResponseSchema, userResponseSchema } from "@/schemas";
import type { PaginatedUserResponse, UserResponse } from "@/lib/api/gen/types.gen";

/** Server-side: fetch users page. Pass accessToken explicitly or leave blank to resolve from session. */
export async function getUsersPage(page = 1, perPage = 8, accessToken?: string) {
  return api.get<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
    accessToken,
  );
}

/** Server-side: fetch single user by ID. */
export async function getUser(id: string, accessToken?: string) {
  return api.get<UserResponse>(`/users/${id}`, userResponseSchema, accessToken);
}
