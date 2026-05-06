import { NextRequest, NextResponse } from "next/server";

const env = process.env.ENVIRONMENT ?? "production";
const isLax = env === "local" || env === "test" || env === "development";

export function proxy(request: NextRequest) {
  // Only local, test, and development skip strict CSP to preserve dev experience.
  // Staging and production must enforce nonce-based CSP.
  if (isLax) {
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

  // Propagate nonce and CSP to downstream Server Components via copied request headers.
  // Next.js framework uses the request-side CSP header to auto-inject nonces into
  // framework scripts and inline styles.
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("Content-Security-Policy", csp);

  const response = NextResponse.next({
    request: {
      headers: requestHeaders,
    },
  });
  response.headers.set("Content-Security-Policy", csp);
  return response;
}

export const config = {
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico).*)"],
};
