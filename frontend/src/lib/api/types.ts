export type {
  UserResponse as User,
  PaginatedUserResponse,
  CreateUserInput as CreateUserRequest,
  UpdateUserInput as UpdateUserRequest,
} from "@/schemas";

import type { components } from "./schema.d.ts";
export type ErrorResponse = components["schemas"]["ErrorResponse"];
