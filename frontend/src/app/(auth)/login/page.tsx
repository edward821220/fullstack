"use client";

import { signIn, useSession } from "next-auth/react";
import { useEffect } from "react";

export default function LoginPage() {
  const { status } = useSession();

  useEffect(() => {
    if (status === "unauthenticated") {
      signIn("oidc", { callbackUrl: "/dashboard" });
    }
  }, [status]);

  if (status === "loading") {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-sm text-gray-500">Redirecting to identity provider...</p>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center">
      <p className="text-sm text-gray-500">Redirecting...</p>
    </div>
  );
}
