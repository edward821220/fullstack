import { clientFetch, clientMutate, serverFetch } from "@/lib/api/fetcher";
import { serverGet } from "@/lib/api/server";
import { paginatedUserResponseSchema, userResponseSchema } from "@/schemas";
import type {
  PaginatedUserResponse,
  UserResponse,
  CreateUserInput,
  UpdateUserInput,
} from "@/schemas";

/** Server-side with explicit token: fetch users page (for SSR/RSC where token is passed manually). */
export async function fetchUsersPage(accessToken: string | undefined, page = 1, perPage = 8) {
  return serverFetch<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
    accessToken,
  );
}

/** Server-side with auto token: fetch users page (for Server Components — token resolved from session). */
export async function getUsersServer(page = 1, perPage = 8) {
  return serverGet<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
  );
}

/** Client-side: fetch users page via axios (token auto-attached by interceptor). Used by SWR hooks. */
export async function getUsersPage(page = 1, perPage = 20) {
  return clientFetch<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
  );
}

/** Fetch single user by ID (client-side). */
export async function getUser(id: string) {
  return clientFetch<UserResponse>(`/users/${id}`, userResponseSchema);
}

/** Create user (client-side). */
export async function createUser(input: CreateUserInput) {
  return clientMutate<UserResponse>("/users", userResponseSchema, "POST", input);
}

/** Update user (client-side). */
export async function updateUser(id: string, input: UpdateUserInput) {
  return clientMutate<UserResponse>(`/users/${id}`, userResponseSchema, "PUT", input);
}

/** Delete user (client-side). */
export async function deleteUser(id: string) {
  return clientMutate<void>(`/users/${id}`, undefined, "DELETE");
}
