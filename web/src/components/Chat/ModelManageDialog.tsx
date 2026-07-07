import { useState, useMemo, useRef, useEffect, useCallback } from "react";
import { Search, X, Check } from "lucide-react";
import type { ProviderInfo, FlatModel } from "../../types/events";
import { PROVIDER_DISPLAY_NAMES } from "./ModelSelector";
import { ApiKeyDialog } from "./ApiKeyDialog";

interface ModelManageDialogProps {
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelSelect: (modelId: string, provider: string) => void;
  onClose: () => void;
  baseUrl: string;
  onProvidersChange?: () => void;
}

export function ModelManageDialog({
  providers,
  selectedModel,
  selectedProvider,
  onModelSelect,
  onClose,
  baseUrl,
  onProvidersChange,
}: ModelManageDialogProps) {
  const [query, setQuery] = useState("");
  const [keyRequestProvider, setKeyRequestProvider] = useState<string | null>(null);
  const [hoveredKey, setHoveredKey] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);

  const HIDDEN_KEY = "pick_hidden_models";

  function getHiddenSet(): Set<string> {
    try {
      const raw = localStorage.getItem(HIDDEN_KEY);
      if (!raw) return new Set();
      return new Set(JSON.parse(raw) as string[]);
    } catch {
      return new Set();
    }
  }

  function toggleHidden(key: string) {
    const set = getHiddenSet();
    if (set.has(key)) set.delete(key);
    else set.add(key);
    localStorage.setItem(HIDDEN_KEY, JSON.stringify(Array.from(set)));
    setTick((t) => t + 1);
  }

  function ensureVisible(key: string) {
    const set = getHiddenSet();
    if (!set.has(key)) return;
    set.delete(key);
    localStorage.setItem(HIDDEN_KEY, JSON.stringify(Array.from(set)));
    setTick((t) => t + 1);
  }

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const hiddenSet = useMemo(() => getHiddenSet(), [tick]);

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
    return flatAll.filter((m) => m.searchText.includes(q));
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
      ensureVisible(`${item.provider}/${item.id}`);
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
        className="bg-[var(--surface-base)] border border-[var(--border-base)] rounded-xl shadow-xl w-[400px] h-[50vh] flex flex-col overflow-hidden"
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
                    const itemKey = `${item.provider}/${item.id}`;
                    const selected = item.id === selectedModel && item.provider === selectedProvider;
                    const hovered = hoveredKey === itemKey;
                    return (
                      <div
                        key={itemKey}
                        className="flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs rounded-md transition-colors"
                        style={{
                          backgroundColor: selected
                            ? "color-mix(in oklab, var(--accent-primary) 12%, var(--surface-base))"
                            : hovered
                              ? "var(--surface-hover)"
                              : "transparent",
                          color: hasKey ? "var(--text-primary)" : "var(--text-muted)",
                        }}
                        onClick={() => hasKey ? handleSelect(item) : setKeyRequestProvider(item.provider)}
                        onMouseEnter={() => setHoveredKey(itemKey)}
                        onMouseLeave={() => setHoveredKey(null)}
                      >
                        <span className="truncate flex-1">{item.name}</span>
                        {hasKey && (
                          <button
                            onClick={(e) => { e.stopPropagation(); toggleHidden(itemKey); }}
                            disabled={selected}
                            className={`shrink-0 p-1 rounded transition-colors ${
                              selected ? "opacity-30 cursor-not-allowed" : "cursor-pointer hover:bg-[var(--surface-hover)]"
                            }`}
                            title={hiddenSet.has(itemKey) ? "Show in quick selector" : "Hide from quick selector"}
                          >
                            <span
                              className="block w-5 h-5 rounded-full border-2 transition-colors"
                              style={{
                                backgroundColor: hiddenSet.has(itemKey) ? "transparent" : "#22c55e",
                                borderColor: hiddenSet.has(itemKey) ? "#64748b" : "#22c55e",
                              }}
                            />
                          </button>
                        )}
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
      </div>

      {keyRequestProvider && (
        <ApiKeyDialog
          provider={keyRequestProvider}
          baseUrl={baseUrl}
          onClose={() => setKeyRequestProvider(null)}
          onSuccess={() => {
            onProvidersChange?.();
          }}
        />
      )}
    </div>
  );
}
