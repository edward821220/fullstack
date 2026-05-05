import type { z } from "zod/v4";

export function parse<T>(data: unknown, schema: z.ZodSchema<T>): T {
  const result = schema.safeParse(data);
  if (!result.success) {
    throw new Error(`API response validation failed: ${result.error.message}`);
  }
  return result.data;
}
