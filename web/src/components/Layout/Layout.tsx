import type { ReactNode } from "react";

interface LayoutProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  leftPanel: ReactNode;
  children: ReactNode;
}

export function Layout({
  sidebarOpen,
  onToggleSidebar,
  leftPanel,
  children,
}: LayoutProps) {
  return (
    <div className="flex h-screen overflow-hidden bg-neutral-950 text-neutral-100">
      {/* Mobile overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50 md:hidden"
          onClick={onToggleSidebar}
        />
      )}

      {/* Left Panel */}
      <aside
        className={`${
          sidebarOpen ? "translate-x-0" : "-translate-x-full"
        } fixed md:static z-40 h-full transition-transform duration-200 ease-in-out
        w-[80vw] max-w-[280px] md:w-[10%] md:min-w-[180px] md:max-w-[280px]
        flex flex-col border-r border-neutral-800 bg-neutral-900`}
      >
        {leftPanel}
      </aside>

      {/* Toggle button */}
      <button
        onClick={onToggleSidebar}
        className="fixed top-3 z-50 p-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors md:top-3"
        style={{
          left: sidebarOpen ? "calc(max(10%, 180px) + 8px)" : "8px",
        }}
      >
        <svg
          className="w-4 h-4"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          {sidebarOpen ? (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          ) : (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 6h16M4 12h16M4 18h16"
            />
          )}
        </svg>
      </button>

      {/* Right Panel */}
      <main className="flex-1 flex flex-col min-w-0">{children}</main>
    </div>
  );
}
