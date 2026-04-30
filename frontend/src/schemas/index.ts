import { z } from "zod/v4";

export const createUserSchema = z.object({
  email: z.string().email("Invalid email address"),
  display_name: z.string().min(1, "Display name is required").max(100),
});

export const updateUserSchema = z.object({
  display_name: z.string().min(1).max(100).optional(),
});

export type CreateUserInput = z.infer<typeof createUserSchema>;
export type UpdateUserInput = z.infer<typeof updateUserSchema>;
