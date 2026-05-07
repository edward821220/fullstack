import type { Metadata } from "next";
import { headers } from "next/headers";
import { Providers } from "@/components/features/providers";
import "@/styles/globals.css";

export const metadata: Metadata = {
  title: "Fullstack Template",
  description: "Fullstack template with Rust backend and Next.js frontend",
};

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  const h = await headers();
  const nonce = h.get("x-nonce") || undefined;

  return (
    <html lang="zh-TW" suppressHydrationWarning nonce={nonce}>
      <body className="min-h-screen bg-background text-foreground antialiased" nonce={nonce}>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
