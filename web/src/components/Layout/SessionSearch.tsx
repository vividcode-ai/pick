import { Search, X } from "lucide-react";

interface SessionSearchProps {
  query: string;
  onQueryChange: (q: string) => void;
}

export function SessionSearch({ query, onQueryChange }: SessionSearchProps) {
  return (
    <div className="relative px-3 py-2">
      <Search className="absolute left-5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--text-muted)]" />
      <input
        type="text"
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        placeholder="Search sessions..."
        className="w-full pl-8 pr-7 py-1.5 text-xs rounded-md bg-[var(--surface-search)] border border-[var(--border-base)] text-[var(--text-primary)] placeholder-[var(--text-muted)] outline-none focus:border-[var(--text-muted)] transition-colors"
      />
      {query && (
        <button
          onClick={() => onQueryChange("")}
          className="absolute right-4 top-1/2 -translate-y-1/2 text-[var(--text-muted)] hover:text-[var(--text-secondary)]"
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}
