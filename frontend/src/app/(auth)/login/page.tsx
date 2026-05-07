"use client";

import { signIn, useSession } from "next-auth/react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export default function LoginPage() {
  const { status } = useSession();
  const router = useRouter();
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (status === "authenticated") {
      router.replace("/dashboard");
    }
  }, [router, status]);

  if (status === "loading") {
    return (
      <Card className="w-full max-w-md shadow-lg shadow-slate-900/5">
        <CardHeader>
          <CardTitle>Checking your session</CardTitle>
          <CardDescription>We are validating your authentication status.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="h-2 rounded-full bg-secondary">
            <div className="h-2 w-2/3 rounded-full bg-primary" />
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="w-full max-w-md shadow-xl shadow-slate-900/5">
      <CardHeader className="space-y-3">
        <div className="inline-flex w-fit rounded-full bg-accent px-3 py-1 text-xs font-medium text-accent-foreground">
          Bank on-prem starter
        </div>
        <CardTitle className="text-2xl">Sign in with your identity provider</CardTitle>
        <CardDescription>
          This template is wired for OIDC-based SSO so teams can start from a protected baseline.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="rounded-xl border border-border bg-background px-4 py-3 text-sm leading-6 text-muted-foreground">
          Use your bank or local development IdP account to continue into the protected dashboard.
        </div>
        <Button
          className="w-full"
          onClick={async () => {
            setIsSubmitting(true);
            await signIn("oidc", { callbackUrl: "/dashboard" });
            setIsSubmitting(false);
          }}
          size="lg"
        >
          {isSubmitting ? "Redirecting to SSO..." : "Continue with SSO"}
        </Button>
      </CardContent>
    </Card>
  );
}
