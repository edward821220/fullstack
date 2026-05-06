import { cookies } from "next/headers";
import { getToken } from "next-auth/jwt";
import type { z } from "zod/v4";
import { parse } from "@/lib/api/parse";

const API_BASE = process.env.API_BASE_URL ?? "http://localhost:3001/api/v1";

/** Get the access token from the encrypted JWT cookie (server-side only). */
export async function getServerAccessToken(): Promise<string | undefined> {
  const cookieStore = await cookies();
  const token = await getToken({
    req: {
      headers: {
        cookie: cookieStore.toString(),
      },
    } as unknown as Parameters<typeof getToken>[0]["req"],
    secret: process.env.NEXTAUTH_SECRET,
  });
  return (token?.accessToken as string | undefined) ?? undefined;
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
  const response = await fetch(`${API_BASE}${endpoint}`, {
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
  const response = await fetch(`${API_BASE}${endpoint}`, {
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
  const response = await fetch(`${API_BASE}${endpoint}`, {
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
  const response = await fetch(`${API_BASE}${endpoint}`, {
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
