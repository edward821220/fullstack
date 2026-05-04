import type { z } from "zod/v4";
import apiClient from "@/lib/api/client";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001/api/v1";

export function parse<T>(data: unknown, schema: z.ZodSchema<T>): T {
  const result = schema.safeParse(data);
  if (!result.success) {
    throw new Error(`API response validation failed: ${result.error.message}`);
  }
  return result.data;
}

/** Server-side generic fetch with Zod validation (for SSR/RSC). Pass accessToken explicitly. */
export async function serverFetch<T>(
  endpoint: string,
  schema: z.ZodSchema<T>,
  accessToken?: string,
): Promise<T> {
  const response = await fetch(`${API_BASE}${endpoint}`, {
    cache: "no-store",
    headers: {
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch ${endpoint}: ${response.status}`);
  }

  return parse(await response.json(), schema);
}

/** Client-side generic fetch with Zod validation (for SWR/client components). Token auto-attached by axios interceptor. */
export async function clientFetch<T>(endpoint: string, schema: z.ZodSchema<T>): Promise<T> {
  const res = await apiClient.get<unknown>(endpoint);
  return parse(res.data, schema);
}
