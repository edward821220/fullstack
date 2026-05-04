import { z } from "zod/v4";

export const userResponseSchema = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  display_name: z.string(),
  role: z.string(),
  email_verified: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});

export const paginatedUserResponseSchema = z.object({
  data: z.array(userResponseSchema),
  page: z.number().int(),
  per_page: z.number().int(),
  total: z.number().int(),
});

export const createUserSchema = z.object({
  email: z.string().email("Invalid email address"),
  display_name: z.string().min(1, "Display name is required").max(100),
});

export const updateUserSchema = z.object({
  display_name: z.string().min(1).max(100).optional(),
});

export type UserResponse = z.infer<typeof userResponseSchema>;
export type PaginatedUserResponse = z.infer<typeof paginatedUserResponseSchema>;
export type CreateUserInput = z.infer<typeof createUserSchema>;
export type UpdateUserInput = z.infer<typeof updateUserSchema>;
