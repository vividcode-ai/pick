import type { ReactNode } from "react";

interface LayoutProps {
  sidebar?: ReactNode;
  children: ReactNode;
  mobileNav?: ReactNode;
}

export function Layout({ sidebar, children, mobileNav }: LayoutProps) {
  return (
    <div className="flex h-screen overflow-hidden">
      {/* Desktop sidebar */}
      {sidebar && (
        <aside className="hidden md:flex w-64 flex-col border-r border-neutral-800 bg-neutral-900">
          {sidebar}
        </aside>
      )}

      {/* Main content */}
      <main className="flex-1 flex flex-col min-w-0">{children}</main>

      {/* Mobile bottom nav */}
      {mobileNav && (
        <nav className="md:hidden fixed bottom-0 left-0 right-0 z-50 border-t border-neutral-800 bg-neutral-900">
          {mobileNav}
        </nav>
      )}
    </div>
  );
}

export function SidebarHeader({ children }: { children: ReactNode }) {
  return (
    <div className="px-4 py-3 border-b border-neutral-800">
      <h1 className="text-lg font-semibold text-neutral-100">{children}</h1>
    </div>
  );
}

export function SidebarContent({ children }: { children: ReactNode }) {
  return <div className="flex-1 overflow-y-auto px-2 py-2">{children}</div>;
}
