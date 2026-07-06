import { useState, useMemo, useRef, useEffect, useCallback } from "react";
import { Search, X, Check } from "lucide-react";
import fuzzysort from "fuzzysort";
import type { ProviderInfo, FlatModel } from "../../types/events";
import { PROVIDER_DISPLAY_NAMES } from "./ModelSelector";

interface ModelManageDialogProps {
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelSelect: (modelId: string, provider: string) => void;
  onClose: () => void;
}

export function ModelManageDialog({
  providers,
  selectedModel,
  selectedProvider,
  onModelSelect,
  onClose,
}: ModelManageDialogProps) {
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setTimeout(() => searchRef.current?.focus(), 80);
  }, []);

  const flatAll = useMemo(() => {
    return providers.flatMap((p) =>
      p.models.map((m) => ({
        ...m,
        provider: p.provider,
        providerDisplayName: PROVIDER_DISPLAY_NAMES[p.provider] || p.provider,
        searchText: `${m.name} ${PROVIDER_DISPLAY_NAMES[p.provider] || p.provider} ${m.id}`.toLowerCase(),
      }))
    );
  }, [providers]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return flatAll;
    return fuzzysort.go(q, flatAll, { keys: ["searchText"], threshold: -10000 }).map((r) => r.obj);
  }, [flatAll, query]);

  const providerMap = useMemo(() => {
    const map = new Map(providers.map((p) => [p.provider, p]));
    return map;
  }, [providers]);

  const grouped = useMemo(() => {
    const groups = new Map<string, FlatModel[]>();
    for (const m of filtered) {
      const cat = m.providerDisplayName;
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat)!.push(m);
    }
    return Array.from(groups.entries())
      .map(([cat, items]) => ({
        category: cat,
        items: items.sort((a, b) => a.name.localeCompare(b.name)),
      }))
      .sort((a, b) => a.category.localeCompare(b.category));
  }, [filtered]);

  const handleSelect = useCallback(
    (item: FlatModel) => {
      onModelSelect(item.id, item.provider);
      onClose();
    },
    [onModelSelect, onClose]
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="bg-[var(--surface-base)] border border-[var(--border-base)] rounded-xl shadow-xl w-[400px] max-h-[80vh] flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-base)]">
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">Model Manager</h2>
          <button
            onClick={onClose}
            className="p-1 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Search */}
        <div className="flex items-center gap-1 px-4 pt-3 pb-2">
          <div className="relative flex-1">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-[var(--text-muted)] pointer-events-none" />
            <input
              ref={searchRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search models..."
              className="w-full pl-7 pr-2 py-1.5 text-xs border border-[var(--border-base)] rounded-md bg-[var(--surface-base)] text-[var(--text-primary)] outline-none focus:border-[var(--accent-primary)] placeholder-[var(--text-muted)]"
              spellCheck={false}
              autoComplete="off"
              autoCorrect="off"
            />
          </div>
        </div>

        {/* List */}
        <div className="flex-1 overflow-y-auto px-2 pb-2 min-h-0">
          {grouped.length === 0 ? (
            <div className="px-3 py-6 text-center text-xs text-[var(--text-muted)]">
              {query ? `No models found for "${query}"` : "No models available"}
            </div>
          ) : (
            grouped.map((group) => {
              const rawProvider = providerMap.get(
                Object.entries(PROVIDER_DISPLAY_NAMES).find(([, v]) => v === group.category)?.[0] ?? ""
              );
              const hasKey = rawProvider?.has_key ?? false;
              return (
                <div key={group.category}>
                  <div className="sticky top-0 z-[1] flex items-center gap-2 px-3 py-1.5 text-[10px] font-semibold text-[var(--text-muted)] uppercase tracking-wider bg-[var(--surface-base)] border-b border-[var(--border-base)]">
                    <span
                      className={`w-2 h-2 rounded-full shrink-0 ${hasKey ? "" : "opacity-30"}`}
                      style={{ backgroundColor: hasKey ? "#22c55e" : "#64748b" }}
                    />
                    <span>{group.category}</span>
                    {!hasKey && (
                      <span className="text-[10px] text-[var(--text-muted)] normal-case font-normal ml-1">
                        (No API key)
                      </span>
                    )}
                  </div>
                  {group.items.map((item) => {
                    const selected = item.id === selectedModel && item.provider === selectedProvider;
                    return (
                      <div
                        key={`${item.provider}/${item.id}`}
                        className="flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs rounded-md hover:bg-[var(--surface-hover)] transition-colors"
                        style={{
                          backgroundColor: selected
                            ? "color-mix(in oklab, var(--accent-primary) 12%, var(--surface-base))"
                            : "transparent",
                          color: hasKey ? "var(--text-primary)" : "var(--text-muted)",
                        }}
                        onClick={() => hasKey && handleSelect(item)}
                      >
                        <span className="truncate flex-1">{item.name}</span>
                        {selected && (
                          <Check className="w-3 h-3 shrink-0 text-[var(--accent-primary)]" />
                        )}
                      </div>
                    );
                  })}
                </div>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-2.5 border-t border-[var(--border-base)] flex justify-end">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded-md bg-[var(--surface-button)] text-[var(--text-primary)] hover:opacity-80 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
