import { getServerSession } from "next-auth";
import { authOptions } from "@/lib/auth/config";
import type { z } from "zod/v4";
import { parse } from "@/lib/api/parse";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001/api/v1";
const BACKEND_INTERNAL = process.env.BACKEND_INTERNAL_URL ?? API_BASE;

/** Get the access token from the server-side session. */
export async function getServerAccessToken(): Promise<string | undefined> {
  const session = await getServerSession(authOptions);
  return session?.accessToken ?? undefined;
}

async function resolveToken(accessToken?: string): Promise<string | undefined> {
  return accessToken ?? (await getServerAccessToken());
}

/** Server-side GET with Zod validation and optional explicit token. */
export async function get<T>(
  endpoint: string,
  schema: z.ZodSchema<T>,
  accessToken?: string,
): Promise<T> {
  const token = await resolveToken(accessToken);
  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    cache: "no-store",
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Server GET ${endpoint} failed (${response.status}): ${body}`);
  }

  return parse(await response.json(), schema);
}

/** Server-side POST with Zod validation and optional explicit token. */
export async function post<T>(
  endpoint: string,
  body: unknown,
  schema: z.ZodSchema<T>,
  accessToken?: string,
): Promise<T> {
  const token = await resolveToken(accessToken);
  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    method: "POST",
    cache: "no-store",
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`Server POST ${endpoint} failed (${response.status}): ${errorBody}`);
  }

  return parse(await response.json(), schema);
}

/** Server-side PUT with Zod validation and optional explicit token. */
export async function put<T>(
  endpoint: string,
  body: unknown,
  schema: z.ZodSchema<T>,
  accessToken?: string,
): Promise<T> {
  const token = await resolveToken(accessToken);
  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    method: "PUT",
    cache: "no-store",
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`Server PUT ${endpoint} failed (${response.status}): ${errorBody}`);
  }

  return parse(await response.json(), schema);
}

/** Server-side DELETE with optional Zod validation and optional explicit token. */
export async function del<T>(
  endpoint: string,
  schema: z.ZodSchema<T> | undefined,
  accessToken?: string,
): Promise<T> {
  const token = await resolveToken(accessToken);
  const response = await fetch(`${BACKEND_INTERNAL}${endpoint}`, {
    method: "DELETE",
    cache: "no-store",
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      "Content-Type": "application/json",
    },
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(`Server DELETE ${endpoint} failed (${response.status}): ${errorBody}`);
  }

  if (response.status === 204) {
    return undefined as unknown as T;
  }

  if (schema) {
    return parse(await response.json(), schema);
  }
  return (await response.json()) as T;
}
