import { clientFetch, serverFetch } from "@/lib/api/fetcher";
import { paginatedUserResponseSchema } from "@/schemas";
import type { PaginatedUserResponse } from "@/schemas";

/** Server-side: fetch users page with explicit token (for SSR/RSC). */
export async function fetchUsersPage(accessToken: string | undefined, page = 1, perPage = 8) {
  return serverFetch<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
    accessToken,
  );
}

/** Client-side: fetch users page via axios (token auto-attached by interceptor). Used by SWR hooks. */
export async function getUsersPage(page = 1, perPage = 20) {
  return clientFetch<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
  );
}
