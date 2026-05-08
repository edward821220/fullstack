import { getToken } from "next-auth/jwt";
import { NextRequest, NextResponse } from "next/server";

const BACKEND_URL = process.env.BACKEND_INTERNAL_URL ?? "http://localhost:3001/api/v1";
const TRUSTED_ORIGIN = process.env.NEXTAUTH_URL ?? "http://localhost:3000";
const PROXY_TIMEOUT_MS = 30_000;

const ALLOWED_RESPONSE_HEADERS = new Set([
  "content-type",
  "content-length",
  "cache-control",
  "etag",
  "last-modified",
  "x-request-id",
  "x-total-count",
  "location",
]);

function isMutatingMethod(method: string): boolean {
  return ["POST", "PUT", "PATCH", "DELETE"].includes(method);
}

function isSameOriginRequest(request: NextRequest): boolean {
  // Prefer Sec-Fetch-Site header (modern browsers)
  const secFetchSite = request.headers.get("sec-fetch-site");
  if (secFetchSite === "same-origin") {
    return true;
  }
  // Fallback to Origin header check
  const origin = request.headers.get("origin");
  if (origin) {
    return origin === TRUSTED_ORIGIN;
  }
  return false;
}

async function proxy(request: NextRequest, { params }: { params: Promise<{ path?: string[] }> }) {
  // CSRF gate: fail closed for cookie-authenticated state changes
  if (isMutatingMethod(request.method) && !isSameOriginRequest(request)) {
    return NextResponse.json({ error: "CSRF check failed: origin mismatch" }, { status: 403 });
  }

  const token = await getToken({
    req: request,
    secret: process.env.NEXTAUTH_SECRET,
  });

  const { path: pathSegments = [] } = await params;
  const path = pathSegments.join("/");
  const search = request.nextUrl.search;
  const url = `${BACKEND_URL}/${path}${search}`;

  const headers = new Headers(request.headers);
  headers.delete("host");
  headers.delete("content-length");

  if (token?.accessToken) {
    headers.set("Authorization", `Bearer ${token.accessToken as string}`);
  }

  const body =
    request.method !== "GET" && request.method !== "HEAD" ? await request.arrayBuffer() : undefined;

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), PROXY_TIMEOUT_MS);

  try {
    const response = await fetch(url, {
      method: request.method,
      headers,
      body,
      signal: controller.signal,
    });

    const filteredHeaders = new Headers();
    response.headers.forEach((value, key) => {
      if (ALLOWED_RESPONSE_HEADERS.has(key.toLowerCase())) {
        filteredHeaders.set(key, value);
      }
    });

    return new NextResponse(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: filteredHeaders,
    });
  } finally {
    clearTimeout(timeoutId);
  }
}

export const GET = proxy;
export const POST = proxy;
export const PUT = proxy;
export const DELETE = proxy;
export const PATCH = proxy;
