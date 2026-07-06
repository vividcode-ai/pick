import { useState, useRef, useEffect, useMemo, useCallback } from "react";
import { ChevronDown, Check, Plus, Clock, Search } from "lucide-react";
import fuzzysort from "fuzzysort";
import type { ProviderInfo, FlatModel } from "../../types/events";
import { openSettings } from "../../stores/settings";
import { ModelTooltip } from "./ModelTooltip";

interface ModelSelectorProps {
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelChange: (modelId: string, provider: string) => void;
  disabled?: boolean;
}

const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  "anthropic": "Anthropic",
  "amazon-bedrock": "Amazon Bedrock",
  "azure-openai-responses": "Azure OpenAI",
  "cerebras": "Cerebras",
  "cloudflare-ai-gateway": "Cloudflare AI Gateway",
  "cloudflare-workers-ai": "Cloudflare Workers AI",
  "deepseek": "DeepSeek",
  "fireworks": "Fireworks",
  "google": "Google Gemini",
  "google-vertex": "Google Vertex AI",
  "groq": "Groq",
  "huggingface": "Hugging Face",
  "kimi-coding": "Kimi For Coding",
  "mistral": "Mistral",
  "minimax": "MiniMax",
  "moonshotai": "Moonshot AI",
  "opencode": "OpenCode Zen",
  "opencode-go": "OpenCode Go",
  "openai": "OpenAI",
  "openrouter": "OpenRouter",
  "together": "Together AI",
  "vercel-ai-gateway": "Vercel AI Gateway",
  "xai": "xAI",
  "zai": "Z.AI",
  "nvidia": "NVIDIA",
  "xiaomi": "Xiaomi MiMo",
};

const PROVIDER_COLORS: Record<string, string> = {
  "anthropic": "#d4a574",
  "openai": "#00a67e",
  "deepseek": "#4f6ef7",
  "google": "#4285f4",
  "github-copilot": "#6e40c9",
  "mistral": "#ffb347",
  "groq": "#f97316",
  "together": "#8b5cf6",
  "openrouter": "#64748b",
  "perplexity": "#1a1a2e",
  "xai": "#141414",
  "meta": "#0668e1",
  "cohere": "#39594d",
  "fireworks": "#f43f5e",
  "cerebras": "#10b981",
  "nvidia": "#76b900",
  "xiaomi": "#ff6900",
};

const RECENT_MODELS_KEY = "pick_recent_models";
const RECENT_LIMIT = 5;

function getProviderColor(provider: string): string {
  return PROVIDER_COLORS[provider] || "#64748b";
}

function getRecentModels(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_MODELS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveRecentModels(ids: string[]) {
  localStorage.setItem(RECENT_MODELS_KEY, JSON.stringify(ids.slice(0, RECENT_LIMIT)));
}

function pushRecentModel(provider: string, modelId: string) {
  const key = `${provider}/${modelId}`;
  const recent = getRecentModels();
  const filtered = recent.filter((k) => k !== key);
  filtered.unshift(key);
  saveRecentModels(filtered);
}

export function ModelSelector({
  providers,
  selectedModel,
  selectedProvider,
  onModelChange,
  disabled,
}: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const displayNameCache = useMemo(() => PROVIDER_DISPLAY_NAMES, []);

  const allModels: FlatModel[] = useMemo(() => {
    return providers
      .filter((p) => p.has_key)
      .flatMap((p) =>
        p.models.map((m) => ({
          ...m,
          provider: p.provider,
          providerDisplayName: displayNameCache[p.provider] || p.provider,
          searchText: `${m.name} ${displayNameCache[p.provider] || p.provider} ${m.id}`.toLowerCase(),
        }))
      );
  }, [providers, displayNameCache]);

  const recentKeys = useMemo(() => getRecentModels(), [open]);

  const [searchQuery, setSearchQuery] = useState("");

  const recentSet = useMemo(() => {
    const set = new Set(recentKeys);
    return set;
  }, [recentKeys]);

  const recentModels: FlatModel[] = useMemo(() => {
    const map = new Map(allModels.map((m) => [`${m.provider}/${m.id}`, m]));
    return recentKeys.map((k) => map.get(k)).filter((m): m is FlatModel => !!m);
  }, [allModels, recentKeys]);

  const selectedDetail = useMemo(
    () => allModels.find((m) => m.id === selectedModel && m.provider === selectedProvider) || null,
    [allModels, selectedModel, selectedProvider]
  );

  const handleSelect = useCallback(
    (item: FlatModel | undefined) => {
      if (!item) return;
      onModelChange(item.id, item.provider);
      pushRecentModel(item.provider, item.id);
      setOpen(false);
    },
    [onModelChange]
  );

  const recentKeysRef = useRef(recentKeys);
  recentKeysRef.current = recentKeys;

  return (
    <div className="relative" ref={containerRef}>
      <button
        onClick={() => {
          setOpen((v) => !v);
        }}
        disabled={disabled}
        className="selector-trigger max-w-[140px]"
      >
        {selectedDetail && (
          <span
            className="w-2 h-2 rounded-full shrink-0"
            style={{ backgroundColor: getProviderColor(selectedDetail.provider) }}
          />
        )}
        <span className="selector-trigger-primary">
          {selectedDetail?.name || "Select model"}
        </span>
        <span className="selector-trigger-icon">
          <ChevronDown className="w-3 h-3" />
        </span>
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full left-0 mb-2 selector-popover z-[2200] w-72">
            <div className="flex flex-col min-h-0">
              {/* Search bar */}
              <div className="flex items-center gap-1 px-2 pt-2 pb-1.5 border-b border-[var(--border-base)]">
                <div className="relative flex-1">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-[var(--text-muted)] pointer-events-none" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Search models..."
                    className="w-full pl-7 pr-2 py-1 text-xs border border-[var(--border-base)] rounded-md bg-[var(--surface-base)] text-[var(--text-primary)] outline-none focus:border-[var(--accent-primary)] focus:shadow-[0_0_0_1px_var(--accent-primary)] placeholder-[var(--text-muted)]"
                    autoFocus
                    spellCheck={false}
                    autoComplete="off"
                    autoCorrect="off"
                  />
                </div>
                <button
                  className="p-1 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover)] transition-colors"
                  title="Manage models"
                  onClick={() => { setOpen(false); openSettings("providers"); }}
                  tabIndex={-1}
                >
                  <Plus className="w-3.5 h-3.5" />
                </button>
              </div>

              {/* Results */}
              <ModelListContent
                allModels={allModels}
                searchQuery={searchQuery}
                recentModels={recentModels}
                recentKeys={recentKeys}
                selectedModel={selectedModel}
                selectedProvider={selectedProvider}
                onSelect={handleSelect}
              />
            </div>

            <div className="border-t border-[var(--border-base)]">
              <button
                className="w-full px-3 py-2 text-xs text-center text-[var(--text-muted)] hover:text-[var(--accent-primary)] hover:bg-[var(--surface-hover)] transition-colors border-none bg-transparent cursor-pointer"
                onClick={() => {
                  setOpen(false);
                  openSettings("providers");
                }}
              >
                <Plus className="w-3 h-3 inline mr-1 -mt-0.5" />
                Configure providers
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

// ── Model list content with fuzzy search, recent section, tooltips ──

interface ModelListContentProps {
  allModels: FlatModel[];
  searchQuery: string;
  recentModels: FlatModel[];
  recentKeys: string[];
  selectedModel: string;
  selectedProvider?: string;
  onSelect: (item: FlatModel) => void;
}

function ModelListContent({
  allModels,
  searchQuery,
  recentModels,
  recentKeys,
  selectedModel,
  selectedProvider,
  onSelect,
}: ModelListContentProps) {
  const [activeIdx, setActiveIdx] = useState(0);
  const [mouseActive, setMouseActive] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rowsRef = useRef<Map<string, HTMLDivElement>>(new Map());

  const isSearching = searchQuery.trim().length > 0;

  const filteredModels = useMemo(() => {
    if (!isSearching) return allModels;
    const q = searchQuery.trim().toLowerCase();
    const results = fuzzysort.go(q, allModels, {
      keys: ["searchText"],
      threshold: -10000,
    });
    return results.map((r) => r.obj);
  }, [allModels, searchQuery, isSearching]);

  const recentSet = useMemo(() => new Set(recentKeys), [recentKeys]);

  const groupMap = useMemo(() => {
    const groups = new Map<string, FlatModel[]>();

    if (!isSearching && recentModels.length > 0) {
      groups.set("\x00Recent", recentModels);
    }

    const source = isSearching ? filteredModels : allModels;
    for (const m of source) {
      if (!isSearching && recentSet.has(`${m.provider}/${m.id}`)) continue;
      const cat = m.providerDisplayName;
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat)!.push(m);
    }
    return groups;
  }, [isSearching, recentModels, filteredModels, allModels, recentSet]);

  const sortedGroups = useMemo(() => {
    return Array.from(groupMap.entries())
      .map(([cat, items]) => ({
        category: cat,
        items: items.sort((a, b) => a.name.localeCompare(b.name)),
      }))
      .sort((a, b) => {
        if (a.category === "\x00Recent") return -1;
        if (b.category === "\x00Recent") return 1;
        return a.category.localeCompare(b.category);
      });
  }, [groupMap]);

  const flatList = useMemo(() => sortedGroups.flatMap((g) => g.items), [sortedGroups]);

  const setRowRef = useCallback((key: string, el: HTMLDivElement | null) => {
    if (el) rowsRef.current.set(key, el);
    else rowsRef.current.delete(key);
  }, []);

  useEffect(() => {
    setActiveIdx(0);
  }, [searchQuery]);

  useEffect(() => {
    if (mouseActive || flatList.length === 0) return;
    const scroll = scrollRef.current;
    if (!scroll) return;
    const keys = Array.from(rowsRef.current.keys());
    const key = flatList[activeIdx] ? `${flatList[activeIdx].provider}/${flatList[activeIdx].id}` : null;
    if (!key) return;
    const el = rowsRef.current.get(key);
    if (!el) return;
    const containerRect = scroll.getBoundingClientRect();
    const elRect = el.getBoundingClientRect();
    const top = elRect.top - containerRect.top + scroll.scrollTop;
    const bottom = top + elRect.height;
    if (top < scroll.scrollTop) {
      scroll.scrollTop = top;
    } else if (bottom > scroll.scrollTop + scroll.clientHeight) {
      scroll.scrollTop = bottom - scroll.clientHeight;
    }
  }, [activeIdx, mouseActive, flatList]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const len = flatList.length;
      if (len === 0) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIdx((i) => (i + 1) % len);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIdx((i) => (i - 1 + len) % len);
      } else if (e.key === "Enter") {
        e.preventDefault();
        const item = flatList[activeIdx];
        if (item) onSelect(item);
      } else if (e.key === "Home") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIdx(0);
      } else if (e.key === "End") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIdx(len - 1);
      }
    },
    [flatList, activeIdx, onSelect]
  );

  if (flatList.length === 0) {
    return (
      <div className="px-3 py-6 text-center text-xs text-[var(--text-muted)]">
        {isSearching ? `No models found for "${searchQuery}"` : "No models available"}
      </div>
    );
  }

  return (
    <div ref={scrollRef} className="flex-1 overflow-y-auto max-h-[260px] min-h-0" onKeyDown={handleKeyDown} tabIndex={-1}>
      {sortedGroups.map((group) => {
        const isRecent = group.category === "\x00Recent";
        return (
          <div key={group.category}>
            <div className="sticky top-0 z-[1] px-3 py-1 text-[10px] font-semibold text-[var(--text-muted)] uppercase tracking-wider bg-[var(--surface-base)] border-b border-[var(--border-base)]">
              {isRecent ? (
                <div className="flex items-center gap-1.5 normal-case">
                  <Clock className="w-3 h-3" />
                  <span>Recent</span>
                </div>
              ) : (
                group.category
              )}
            </div>
            {group.items.map((item) => {
              const key = `${item.provider}/${item.id}`;
              const globalIdx = flatList.indexOf(item);
              const selected = item.id === selectedModel && item.provider === selectedProvider;
              const highlighted = globalIdx === activeIdx;
              return (
                <div
                  key={key}
                  ref={(el) => setRowRef(key, el)}
                  className="group relative flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs"
                  style={{
                    backgroundColor: highlighted
                      ? "var(--surface-hover)"
                      : selected
                        ? "color-mix(in oklab, var(--accent-primary) 12%, var(--surface-base))"
                        : "transparent",
                    color: "var(--text-primary)",
                  }}
                  onClick={() => onSelect(item)}
                  onMouseEnter={() => { setMouseActive(true); setActiveIdx(globalIdx); }}
                >
                  <span
                    className="w-2 h-2 rounded-full shrink-0"
                    style={{ backgroundColor: getProviderColor(item.provider) }}
                  />
                  <span className="truncate flex-1">{item.name}</span>
                  {selected && (
                    <Check className="w-3 h-3 shrink-0 text-[var(--accent-primary)]" />
                  )}

                  {/* Tooltip on hover */}
                  <div className="absolute left-full top-0 ml-2 invisible group-hover:visible z-10 pointer-events-none">
                    <div className="bg-[var(--surface-elevated)] border border-[var(--border-base)] rounded-md shadow-lg px-3 py-2 whitespace-nowrap">
                      <ModelTooltip model={item} providerName={item.providerDisplayName} />
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
