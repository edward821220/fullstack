import * as api from "@/lib/api/client";
import { paginatedUserResponseSchema, userResponseSchema } from "@/schemas";
import type {
  PaginatedUserResponse,
  UserResponse,
  CreateUserRequest,
  UpdateUserRequest,
} from "@/lib/api/gen/types.gen";

/** Client-side: fetch users page (SWR). */
export async function getUsersPage(page = 1, perPage = 20) {
  return api.get<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    paginatedUserResponseSchema,
  );
}

/** Client-side: fetch single user by ID. */
export async function getUser(id: string) {
  return api.get<UserResponse>(`/users/${id}`, userResponseSchema);
}

/** Client-side: create user. */
export async function createUser(input: CreateUserRequest) {
  return api.post<UserResponse>("/users", input, userResponseSchema);
}

/** Client-side: update user. */
export async function updateUser(id: string, input: UpdateUserRequest) {
  return api.put<UserResponse>(`/users/${id}`, input, userResponseSchema);
}

/** Client-side: delete user. */
export async function deleteUser(id: string) {
  return api.del<void>(`/users/${id}`, undefined);
}
