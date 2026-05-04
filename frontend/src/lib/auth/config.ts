import type { NextAuthOptions } from "next-auth";
import "./types";

const issuer = process.env.AUTH_OIDC_ISSUER ?? "http://localhost:8080/dex";

const REFRESH_BUFFER_MS = 5 * 60 * 1000;

async function refreshAccessToken(token: { accessToken: string; refreshToken: string }): Promise<{
  accessToken: string;
  refreshToken?: string;
  expiresAt: number;
}> {
  const wellKnownUrl = `${issuer}/.well-known/openid-configuration`;
  const config = await fetch(wellKnownUrl).then((r) => r.json());
  const tokenEndpoint: string = config.token_endpoint;

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
      wellKnown: `${issuer}/.well-known/openid-configuration`,
      authorization: { params: { scope: "openid email profile offline_access" } },
      profile(profile) {
        return {
          id: profile.sub ?? profile.email,
          name: profile.name ?? profile.preferred_username ?? profile.email,
          email: profile.email,
          image: profile.picture ?? null,
        };
      },
    },
  ],
  session: {
    strategy: "jwt",
    maxAge: 24 * 60 * 60, // 24 hours
  },
  callbacks: {
    async jwt({ token, account }) {
      if (account) {
        token.accessToken = account.access_token;
        token.refreshToken = account.refresh_token;
        token.idToken = account.id_token;
        token.expiresAt = account.expires_at ? account.expires_at * 1000 : Date.now() + 3600 * 1000;
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
      session.accessToken = token.accessToken as string | undefined;
      session.error = token.error as string | undefined;
      return session;
    },
  },
  pages: {
    signIn: "/login",
    signOut: "/login",
  },
  events: {
    signOut({ token }) {
      if (token.idToken) {
        const endSessionUrl = new URL(`${issuer}/.well-known/openid-configuration`);
        fetch(endSessionUrl)
          .then((r) => r.json())
          .then((config) => {
            if (config.end_session_endpoint) {
              const url = new URL(config.end_session_endpoint);
              url.searchParams.set("id_token_hint", token.idToken as string);
              url.searchParams.set(
                "post_logout_redirect_uri",
                process.env.AUTH_OIDC_LOGOUT_REDIRECT ??
                  `${process.env.NEXTAUTH_URL ?? "http://localhost:3000"}/login`,
              );
              fetch(url.toString()).catch(() => {});
            }
          })
          .catch(() => {});
      }
    },
  },
};
