import { Search, X } from "lucide-react";

interface SessionSearchProps {
  query: string;
  onQueryChange: (q: string) => void;
}

export function SessionSearch({ query, onQueryChange }: SessionSearchProps) {
  return (
    <div className="relative px-3 py-2">
      <Search className="absolute left-5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-neutral-500" />
      <input
        type="text"
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        placeholder="Search sessions..."
        className="w-full pl-8 pr-7 py-1.5 text-xs rounded-md bg-neutral-800 border border-neutral-700 text-neutral-200 placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
      />
      {query && (
        <button
          onClick={() => onQueryChange("")}
          className="absolute right-4 top-1/2 -translate-y-1/2 text-neutral-500 hover:text-neutral-300"
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}
