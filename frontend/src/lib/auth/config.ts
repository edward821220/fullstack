import type { NextAuthOptions } from "next-auth";
import "./types";

const REFRESH_BUFFER_MS = 5 * 60 * 1000;

function validateAuthConfig() {
  const env = process.env.ENVIRONMENT ?? "production";
  const isLax = env === "local" || env === "test" || env === "development";
  if (isLax) {
    // Local/test/development: skip strict validation to preserve dev experience.
    // Staging and production must enforce fail-closed validation.
    return;
  }

  // During `next build`, the auth config module may be evaluated without runtime secrets.
  // Next.js sets NEXT_PHASE to 'phase-production-build' during the build process.
  const buildPhases = ["phase-production-build", "phase-export"];
  const isBuildPhase = buildPhases.includes(process.env.NEXT_PHASE ?? "");
  if (isBuildPhase) {
    return;
  }

  // Runtime production validation: must be fail-closed.
  const nextauthSecret = process.env.NEXTAUTH_SECRET;
  if (!nextauthSecret || nextauthSecret.length < 32) {
    throw new Error("NEXTAUTH_SECRET must be set and at least 32 characters long in production.");
  }

  const oidcId = process.env.AUTH_OIDC_ID;
  const oidcSecret = process.env.AUTH_OIDC_SECRET;
  if (!oidcId || oidcId.trim().length === 0) {
    throw new Error("AUTH_OIDC_ID must be set in production.");
  }
  if (!oidcSecret || oidcSecret.trim().length === 0) {
    throw new Error("AUTH_OIDC_SECRET must be set in production.");
  }

  const oidcIssuer = process.env.AUTH_OIDC_ISSUER;
  if (!oidcIssuer || oidcIssuer.trim().length === 0) {
    throw new Error("AUTH_OIDC_ISSUER must be set in production.");
  }
  const isLocalhost = oidcIssuer.includes("localhost") || oidcIssuer.includes("127.0.0.1");
  if (!isLocalhost && !oidcIssuer.startsWith("https://")) {
    throw new Error("AUTH_OIDC_ISSUER must use https:// in production (non-localhost).");
  }
}

validateAuthConfig();

const issuer = process.env.AUTH_OIDC_ISSUER ?? "http://localhost:8080/dex";
const wellKnownUrl = `${issuer}/.well-known/openid-configuration`;

// Cache for OIDC discovery metadata to avoid repeated network calls.
// Module-level cache is safe in Next.js because the module is evaluated once per process.
let cachedDiscovery: Record<string, string> | null = null;

async function getDiscoveryConfig(): Promise<Record<string, string>> {
  if (cachedDiscovery) {
    return cachedDiscovery;
  }
  const response = await fetch(wellKnownUrl);
  if (!response.ok) {
    throw new Error(`OIDC discovery failed: ${response.status} ${response.statusText}`);
  }
  const config = (await response.json()) as Record<string, string>;
  cachedDiscovery = config;
  return config;
}

async function refreshAccessToken(token: { accessToken: string; refreshToken: string }): Promise<{
  accessToken: string;
  refreshToken?: string;
  expiresAt: number;
}> {
  const config = await getDiscoveryConfig();
  const tokenEndpoint = config.token_endpoint;
  if (!tokenEndpoint) {
    throw new Error("OIDC discovery did not return token_endpoint");
  }

  const response = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "refresh_token",
      client_id: process.env.AUTH_OIDC_ID ?? "",
      client_secret: process.env.AUTH_OIDC_SECRET ?? "",
      refresh_token: token.refreshToken,
    }),
  });

  if (!response.ok) {
    throw new Error(`Token refresh failed: ${response.status} ${response.statusText}`);
  }

  const tokens = await response.json();

  return {
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    expiresAt: tokens.expires_in ? Date.now() + tokens.expires_in * 1000 : Date.now() + 3600 * 1000,
  };
}

export const authOptions: NextAuthOptions = {
  providers: [
    {
      id: "oidc",
      name: "OIDC",
      type: "oauth",
      clientId: process.env.AUTH_OIDC_ID ?? "",
      clientSecret: process.env.AUTH_OIDC_SECRET ?? "",
      issuer,
      wellKnown: wellKnownUrl,
      authorization: { params: { scope: "openid email profile offline_access" } },
      profile(profile) {
        return {
          id: profile.sub ?? profile.email,
          name: profile.name ?? profile.preferred_username ?? profile.email,
          email: profile.email,
          image: profile.picture ?? null,
          role: profile.role ?? "user",
        };
      },
    },
  ],
  session: {
    strategy: "jwt",
    maxAge: 24 * 60 * 60, // 24 hours
  },
  callbacks: {
    async jwt({ token, account, user }) {
      if (account) {
        token.accessToken = account.access_token;
        token.refreshToken = account.refresh_token;
        token.idToken = account.id_token;
        token.expiresAt = account.expires_at ? account.expires_at * 1000 : Date.now() + 3600 * 1000;
        if (user?.role) {
          token.role = user.role;
        }
        return token;
      }

      if (
        token.accessToken &&
        typeof token.expiresAt === "number" &&
        Date.now() + REFRESH_BUFFER_MS < token.expiresAt
      ) {
        return token;
      }

      if (!token.refreshToken) {
        token.error = "NoRefreshToken";
        return token;
      }

      try {
        const tokens = await refreshAccessToken({
          accessToken: token.accessToken as string,
          refreshToken: token.refreshToken as string,
        });
        token.accessToken = tokens.accessToken;
        token.refreshToken = tokens.refreshToken ?? token.refreshToken;
        token.expiresAt = tokens.expiresAt;
        delete token.error;
      } catch {
        token.error = "RefreshAccessTokenError";
      }

      return token;
    },
    async session({ session, token }) {
      // accessToken is kept server-side only; client code calls backend
      // through Next.js API proxy routes that attach the token on the server.
      session.error = token.error as string | undefined;
      if (token.role && session.user) {
        session.user.role = token.role as string;
      }
      return session;
    },
  },
  pages: {
    signIn: "/login",
    signOut: "/login",
  },
  events: {
    async signOut({ token }) {
      if (!token.idToken) return;

      try {
        const config = await getDiscoveryConfig();
        const endSessionEndpoint = config.end_session_endpoint;
        if (!endSessionEndpoint) return;

        const url = new URL(endSessionEndpoint);
        url.searchParams.set("id_token_hint", token.idToken as string);
        url.searchParams.set(
          "post_logout_redirect_uri",
          process.env.AUTH_OIDC_LOGOUT_REDIRECT ??
            `${process.env.NEXTAUTH_URL ?? "http://localhost:3000"}/login`,
        );
        await fetch(url.toString());
      } catch {
        // Silently ignore RP-initiated logout failures; local session is already cleared.
      }
    },
  },
};
