import { NextRequest, NextResponse } from "next/server";

const isProduction = process.env.NODE_ENV === "production";

export function middleware(request: NextRequest) {
  // For local development, do not apply strict CSP so that inline scripts/styles
  // and eval (e.g. hot reload, devtools) continue to work without nonces.
  if (!isProduction) {
    return NextResponse.next();
  }

  const nonce = Buffer.from(crypto.randomUUID()).toString("base64");

  const csp = [
    "default-src 'self'",
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'`,
    `style-src 'self' 'nonce-${nonce}'`,
    "img-src 'self' data: blob:",
    "font-src 'self'",
    "connect-src 'self'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");

  // Propagate the nonce to downstream Server Components via request headers.
  request.headers.set("x-nonce", nonce);
  const response = NextResponse.next({ request });
  response.headers.set("Content-Security-Policy", csp);
  return response;
}

export const config = {
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico).*)"],
};
