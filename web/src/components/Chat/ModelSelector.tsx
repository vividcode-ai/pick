import { useState, useRef, useEffect, useMemo, useCallback } from "react";
import { ChevronDown, Search, Star, Check, Plus } from "lucide-react";
import type { ProviderInfo } from "../../types/events";
import { toggleFavorite, getFavoriteModelKeys, subscribeToFavorites } from "../../stores/models";

interface FlatModel {
  id: string;
  name: string;
  provider: string;
  hasKey: boolean;
  reasoning: boolean;
  searchText: string;
}

interface ModelSelectorProps {
  providers: ProviderInfo[];
  selectedModel: string;
  onModelChange: (m: string) => void;
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

export function ModelSelector({ providers, selectedModel, onModelChange, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [favoritesRev, setFavoritesRev] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const highlightRef = useRef<number>(0);

  useEffect(() => {
    if (!open) return;
    setTimeout(() => searchRef.current?.focus(), 80);
  }, [open]);

  useEffect(() => {
    return subscribeToFavorites(() => setFavoritesRev((v) => v + 1));
  }, []);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const allModels: FlatModel[] = useMemo(() => {
    return providers.flatMap((p) =>
      p.models.map((m) => ({
        ...m,
        provider: p.provider,
        hasKey: p.has_key,
        searchText: `${m.name} ${p.provider} ${m.id}`.toLowerCase(),
      }))
    );
  }, [providers]);

  const selectedDetail = useMemo(
    () => allModels.find((m) => m.id === selectedModel) || null,
    [allModels, selectedModel]
  );

  const favoriteKeys = useMemo(() => {
    void favoritesRev;
    return getFavoriteModelKeys();
  }, [favoritesRev]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    let list = allModels;
    if (q) {
      list = list.filter((m) => m.searchText.includes(q));
    }
    if (favoritesOnly) {
      list = list.filter((m) => favoriteKeys.has(`${m.provider}/${m.id}`));
    }
    return list;
  }, [allModels, query, favoritesOnly, favoriteKeys]);

  const grouped = useMemo(() => {
    const groups = new Map<string, FlatModel[]>();
    for (const m of filtered) {
      const key = PROVIDER_DISPLAY_NAMES[m.provider] || m.provider;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(m);
    }
    const sorted = Array.from(groups.entries()).sort(([a], [b]) => a.localeCompare(b));
    return sorted;
  }, [filtered]);

  const flatList = useMemo(() => filtered, [filtered]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!open) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        highlightRef.current = Math.min(highlightRef.current + 1, flatList.length - 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        highlightRef.current = Math.max(highlightRef.current - 1, 0);
      } else if (e.key === "Enter" && flatList[highlightRef.current]) {
        e.preventDefault();
        onModelChange(flatList[highlightRef.current].id);
        setOpen(false);
      } else if (e.key === "Escape") {
        setOpen(false);
      }
    },
    [open, flatList, onModelChange]
  );

  const hasFavorites = favoriteKeys.size > 0;

  return (
    <div className="relative" ref={containerRef} onKeyDown={handleKeyDown}>
      <button
        onClick={() => { setOpen((v) => !v); setQuery(""); highlightRef.current = 0; }}
        disabled={disabled || allModels.length === 0}
        className="selector-trigger max-w-[140px]"
      >
        <div className="selector-trigger-label min-w-0">
          <span className="selector-trigger-primary selector-trigger-primary--align-left">
            {selectedDetail?.name || "Model"}
          </span>
          {selectedDetail && (
            <span className="selector-trigger-secondary">
              {PROVIDER_DISPLAY_NAMES[selectedDetail.provider] || selectedDetail.provider}
            </span>
          )}
        </div>
        <span className="selector-trigger-icon">
          <ChevronDown className="w-3 h-3" />
        </span>
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full left-0 mb-2 selector-popover z-[2200]">
            <div className="selector-search-container">
              <div className="selector-input-group">
                <div className="relative flex-1">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-neutral-500" />
                  <input
                    ref={searchRef}
                    type="text"
                    value={query}
                    onChange={(e) => { setQuery(e.target.value); highlightRef.current = 0; }}
                    placeholder="Search models..."
                    className="selector-search-input pl-7"
                  />
                </div>
                <button
                  className="selector-favorites-toggle"
                  data-active={favoritesOnly}
                  disabled={!hasFavorites}
                  onClick={() => setFavoritesOnly((v) => !v)}
                  title={favoritesOnly ? "Show all" : "Favorites only"}
                >
                  <Star className="w-3.5 h-3.5" fill={favoritesOnly ? "currentColor" : "none"} />
                </button>
              </div>
            </div>

            <div className="selector-listbox">
              {grouped.length === 0 ? (
                <div className="selector-empty-state">
                  {query ? "No models found" : "No models available"}
                </div>
              ) : (
                grouped.map(([providerName, models]) => (
                  <div key={providerName}>
                    <div className="selector-group-header">{providerName}</div>
                    {models.map((m) => {
                      const globalIdx = flatList.indexOf(m);
                      const selected = m.id === selectedModel;
                      const fav = favoriteKeys.has(`${m.provider}/${m.id}`);
                      return (
                        <div
                          key={m.id}
                          className="selector-option"
                          data-highlighted={globalIdx === highlightRef.current}
                          data-selected={selected}
                          onClick={() => { onModelChange(m.id); setOpen(false); }}
                          onMouseEnter={() => { highlightRef.current = globalIdx; }}
                        >
                          <div className="selector-option-content">
                            <span className="selector-option-label">{m.name}</span>
                            <span className="selector-option-description">
                              {PROVIDER_DISPLAY_NAMES[m.provider] || m.provider} • {m.provider}/{m.id}
                              {!m.hasKey ? " (no api key)" : ""}
                            </span>
                          </div>
                          <button
                            className="selector-option-star"
                            data-active={fav}
                            onClick={(e) => {
                              e.stopPropagation();
                              toggleFavorite(m.provider, m.id);
                            }}
                            title={fav ? "Remove from favorites" : "Add to favorites"}
                          >
                            <Star className="w-3.5 h-3.5" fill={fav ? "currentColor" : "none"} />
                          </button>
                          {selected && (
                            <span className="selector-option-indicator">
                              <Check className="w-3.5 h-3.5" />
                            </span>
                          )}
                        </div>
                      );
                    })}
                  </div>
                ))
              )}
            </div>

            {!favoritesOnly && hasFavorites && (
              <div className="selector-footer">
                <button
                  className="selector-option-action"
                  onClick={() => setFavoritesOnly(true)}
                >
                  <Star className="w-3 h-3 inline mr-1" fill="currentColor" />
                  Show favorites only
                </button>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
