export default function AuthLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-dvh items-center justify-center bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.12),_transparent_35%),linear-gradient(180deg,_#f8fbff,_#eef4ff)] px-4 py-10">
      {children}
    </div>
  );
}
