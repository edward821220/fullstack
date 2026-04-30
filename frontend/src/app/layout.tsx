import type { Metadata } from "next";
import { Providers } from "@/components/features/providers";
import "@/styles/globals.css";

export const metadata: Metadata = {
  title: "Fullstack Template",
  description: "Fullstack template with Rust backend and Next.js frontend",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-TW" suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground antialiased">
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
