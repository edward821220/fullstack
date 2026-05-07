import axios from "axios";
import type { z } from "zod/v4";
import { parse } from "@/lib/api/parse";

const apiClient = axios.create({
  baseURL: "/api/proxy",
  headers: { "Content-Type": "application/json" },
});

export default apiClient;

/** Client-side GET with Zod validation. Token is attached server-side by the proxy. */
export async function get<T>(endpoint: string, schema: z.ZodSchema<T>): Promise<T> {
  const res = await apiClient.get<unknown>(endpoint);
  return parse(res.data, schema);
}

/** Client-side POST with Zod validation. */
export async function post<T>(endpoint: string, body: unknown, schema: z.ZodSchema<T>): Promise<T> {
  const res = await apiClient.post<unknown>(endpoint, body);
  return parse(res.data, schema);
}

/** Client-side PUT with Zod validation. */
export async function put<T>(endpoint: string, body: unknown, schema: z.ZodSchema<T>): Promise<T> {
  const res = await apiClient.put<unknown>(endpoint, body);
  return parse(res.data, schema);
}

/** Client-side DELETE with optional Zod validation. */
export async function del<T>(endpoint: string, schema?: z.ZodSchema<T>): Promise<T> {
  const res = await apiClient.delete<unknown>(endpoint);
  if (schema) {
    return parse(res.data, schema);
  }
  return res.data as T;
}
