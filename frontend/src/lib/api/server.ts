import { getServerSession } from "next-auth";
import { authOptions } from "@/lib/auth/config";
import type { z } from "zod/v4";
import { parse } from "@/lib/api/fetcher";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001/api/v1";

const BACKEND_INTERNAL = process.env.BACKEND_INTERNAL_URL ?? API_BASE;

/**
 * Get the access token from the server-side session.
 * Use this in Server Components, Route Handlers, and Server Actions.
 */
export async function getServerAccessToken(): Promise<string | undefined> {
  const session = await getServerSession(authOptions);
  return session?.accessToken ?? undefined;
}

/**
 * Server-side API caller that auto-attaches the Bearer token from next-auth session.
 * Use in Server Components for direct backend calls without client-side fetching.
 *
 * @example
 * ```ts
 * const users = await serverGet("/users?page=1&per_page=8", paginatedUserResponseSchema);
 * ```
 */
export async function serverGet<T>(endpoint: string, schema: z.ZodSchema<T>): Promise<T> {
  const accessToken = await getServerAccessToken();

  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    cache: "no-store",
    headers: {
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Server GET ${endpoint} failed (${response.status}): ${body}`);
  }

  return parse(await response.json(), schema);
}

/**
 * Server-side mutation caller for POST/PUT/DELETE with auto-attached Bearer token.
 * Use in Server Actions or Route Handlers.
 */
export async function serverMutate<T>(
  method: "POST" | "PUT" | "DELETE",
  endpoint: string,
  body: unknown,
  schema: z.ZodSchema<T>,
): Promise<T> {
  const accessToken = await getServerAccessToken();

  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    method,
    cache: "no-store",
    headers: {
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`Server ${method} ${endpoint} failed (${response.status}): ${errorBody}`);
  }

  if (response.status === 204) {
    return undefined as unknown as T;
  }

  return parse(await response.json(), schema);
}
