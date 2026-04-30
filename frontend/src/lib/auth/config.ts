import type { NextAuthOptions } from "next-auth";
import "./types";

const issuer = process.env.AUTH_OIDC_ISSUER ?? "http://localhost:8080/dex";

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
      authorization: { params: { scope: "openid email profile" } },
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
  },
  callbacks: {
    async jwt({ token, account }) {
      if (account) {
        token.accessToken = account.access_token;
        token.refreshToken = account.refresh_token;
        token.idToken = account.id_token;
      }
      return token;
    },
    async session({ session, token }) {
      session.accessToken = token.accessToken as string | undefined;
      return session;
    },
  },
  pages: {
    signIn: "/login",
  },
};
