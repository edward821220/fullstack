"use client";

import { useSession } from "next-auth/react";
import { useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

const ERROR_MESSAGES: Record<string, string> = {
  RefreshAccessTokenError: "Your session token could not be refreshed. Please sign in again.",
  NoRefreshToken: "Your session is about to expire and no refresh token is available.",
};

export function AuthError() {
  const { data: session, status } = useSession();

  const handleSignIn = useCallback(async () => {
    const { signIn } = await import("next-auth/react");
    await signIn("oidc", { callbackUrl: "/dashboard" });
  }, []);

  if (status !== "authenticated" || !session?.error) {
    return null;
  }

  const message =
    ERROR_MESSAGES[session.error] ??
    `An authentication error occurred (${session.error}). Please sign in again.`;

  return (
    <Card className="mb-6 border-destructive/50 bg-destructive/5">
      <CardHeader className="pb-2">
        <CardTitle className="text-destructive text-base">Authentication Error</CardTitle>
        <CardDescription>{message}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button variant="outline" size="sm" onClick={handleSignIn}>
          Sign in again
        </Button>
      </CardContent>
    </Card>
  );
}
