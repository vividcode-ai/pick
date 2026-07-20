import { useState, useRef, useEffect, useMemo, useCallback } from "react";
import { ChevronDown, ChevronRight, Plus, Search, Check } from "lucide-react";
import fuzzysort from "fuzzysort";
import type { ProviderInfo, FlatModel } from "../../types/events";
import { ModelManageDialog } from "./ModelManageDialog";

const THINKING_LEVEL_LABELS: Record<string, string> = {
  off: "Off",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "XHigh",
};

interface ModelThinkingSelectorProps {
  providers: ProviderInfo[];
  selectedModel: string;
  selectedProvider?: string;
  onModelChange: (modelId: string, provider: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
  disabled?: boolean;
  baseUrl: string;
  onProvidersChange?: () => void;
  hiddenModels: string[];
  onToggleHidden: (key: string) => void;
  onEnsureVisible: (key: string) => void;
}

export function ModelThinkingSelector({
  providers,
  selectedModel,
  selectedProvider,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
  disabled,
  baseUrl,
  onProvidersChange,
  hiddenModels,
  onToggleHidden,
  onEnsureVisible,
}: ModelThinkingSelectorProps) {
  const [open, setOpen] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);
  const [manageRefreshKey, setManageRefreshKey] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const prevManageOpen = useRef(manageOpen);
  useEffect(() => {
    if (prevManageOpen.current && !manageOpen) {
      setManageRefreshKey((k) => k + 1);
    }
    prevManageOpen.current = manageOpen;
  }, [manageOpen]);

  const allModels: FlatModel[] = useMemo(() => {
    const hidden = new Set(hiddenModels);
    return providers
      .filter((p) => p.has_key)
      .flatMap((p) =>
        p.models.map((m) => ({
          ...m,
          provider: p.provider,
          providerDisplayName: p.provider,
          searchText: `${m.name} ${p.provider} ${m.id}`.toLowerCase(),
        }))
      )
      .filter((m) => !hidden.has(`${m.provider}/${m.id}`));
  }, [providers, manageRefreshKey, hiddenModels]);

  const [searchQuery, setSearchQuery] = useState("");

  const selectedDetail = useMemo(
    () => allModels.find((m) => m.id === selectedModel && m.provider === selectedProvider) || null,
    [allModels, selectedModel, selectedProvider]
  );

  const supportedThinkingLevels = useMemo(() => {
    return selectedDetail?.supported_thinking_levels ?? (
      selectedDetail?.reasoning
        ? ["off", "low", "medium", "high"]
        : ["off"]
    );
  }, [selectedDetail]);

  const effectiveThinkingLevel = supportedThinkingLevels.includes(thinkingLevel)
    ? thinkingLevel
    : "off";

  const selectedLabel = THINKING_LEVEL_LABELS[effectiveThinkingLevel] ?? "Off";

  const handleSelect = useCallback(
    (item: FlatModel | undefined) => {
      if (!item) return;
      onModelChange(item.id, item.provider);
      setOpen(false);
    },
    [onModelChange]
  );

  // ── Thinking level hover popup state ──
  const [thinkHoverKey, setThinkHoverKey] = useState<string | null>(null);
  const [thinkHoverRect, setThinkHoverRect] = useState<DOMRect | null>(null);
  const thinkTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearThinkTimer = useCallback(() => {
    if (thinkTimerRef.current) {
      clearTimeout(thinkTimerRef.current);
      thinkTimerRef.current = null;
    }
  }, []);

  const startThinkTimer = useCallback(() => {
    clearThinkTimer();
    thinkTimerRef.current = setTimeout(() => {
      setThinkHoverKey(null);
      setThinkHoverRect(null);
    }, 150);
  }, [clearThinkTimer]);

  useEffect(() => {
    return () => {
      if (thinkTimerRef.current) clearTimeout(thinkTimerRef.current);
    };
  }, []);

  const handleThinkHover = useCallback((key: string | null, rect: DOMRect | null) => {
    clearThinkTimer();
    setThinkHoverKey(key);
    setThinkHoverRect(rect);
  }, [clearThinkTimer]);

  const handleThinkPopupEnter = useCallback(() => {
    clearThinkTimer();
  }, [clearThinkTimer]);

  const handleThinkPopupLeave = useCallback(() => {
    startThinkTimer();
  }, [startThinkTimer]);

  // Find hovered item for ChevronRight display
  const thinkHoveredModel = useMemo(() => {
    if (!thinkHoverKey) return null;
    return allModels.find((m) => `${m.provider}/${m.id}` === thinkHoverKey) || null;
  }, [allModels, thinkHoverKey]);

  return (
    <div className="relative" ref={containerRef}>
      <button
        onClick={() => { setOpen((v) => !v); setThinkHoverKey(null); setThinkHoverRect(null); }}
        disabled={disabled}
        className="inline-flex items-center gap-1 cursor-pointer text-xs text-[var(--text-primary)] hover:bg-[var(--surface-hover)] rounded-md px-1.5 py-1"
      >
        <span className="selector-trigger-primary">
          {selectedDetail?.name || "Select model"}
        </span>
        {selectedDetail && (
          <span className="text-[10px] text-[var(--text-muted)] ml-0.5">
            {selectedLabel}
          </span>
        )}
        <span className="selector-trigger-icon">
          <ChevronDown className="w-3 h-3" />
        </span>
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => { setOpen(false); setThinkHoverKey(null); setThinkHoverRect(null); }} />
          <div ref={popoverRef} className="absolute bottom-full left-0 mb-2 selector-popover z-[2200] w-72">
            <div className="flex flex-col min-h-0 relative">
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
                  onClick={() => { setOpen(false); setManageOpen(true); }}
                  tabIndex={-1}
                >
                  <Plus className="w-3.5 h-3.5" />
                </button>
              </div>

              <ModelListContent
                allModels={allModels}
                searchQuery={searchQuery}
                selectedModel={selectedModel}
                selectedProvider={selectedProvider}
                onSelect={handleSelect}
                thinkHoverKey={thinkHoverKey}
                onThinkHover={handleThinkHover}
                onStartThinkTimer={startThinkTimer}
              />

              {/* Thinking level popup – rendered outside scroll container */}
              {thinkHoverKey && thinkHoverRect && thinkHoveredModel?.reasoning && (
                <div
                  className="fixed z-[2300]"
                  style={{ left: thinkHoverRect.right + 4, top: thinkHoverRect.top }}
                  onMouseEnter={handleThinkPopupEnter}
                  onMouseLeave={handleThinkPopupLeave}
                >
                  <div className="thinking-sub-popover">
                    <div className="selector-listbox min-w-[90px]" style={{ padding: "0.25rem", maxHeight: "200px", overflowY: "auto" }}>
                      {supportedThinkingLevels.map((value) => {
                        const levelSelected = value === effectiveThinkingLevel;
                        return (
                          <div
                            key={value}
                            className="selector-option"
                            data-selected={levelSelected}
                            onClick={(e) => {
                              e.stopPropagation();
                              onThinkingLevelChange(value);
                            }}
                          >
                            <div className="selector-option-content">
                              <span className="selector-option-label">{THINKING_LEVEL_LABELS[value] ?? value}</span>
                            </div>
                            {levelSelected && (
                              <span className="selector-option-indicator">
                                <Check className="w-3.5 h-3.5" />
                              </span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </>
      )}

      {manageOpen && (
        <ModelManageDialog
          providers={providers}
          selectedModel={selectedModel}
          selectedProvider={selectedProvider}
          onModelSelect={onModelChange}
          onClose={() => setManageOpen(false)}
          baseUrl={baseUrl}
          onProvidersChange={onProvidersChange}
          hiddenModels={hiddenModels}
          onToggleHidden={onToggleHidden}
          onEnsureVisible={onEnsureVisible}
        />
      )}
    </div>
  );
}

// ── Model list content ──

interface ModelListContentProps {
  allModels: FlatModel[];
  searchQuery: string;
  selectedModel: string;
  selectedProvider?: string;
  onSelect: (item: FlatModel) => void;
  thinkHoverKey: string | null;
  onThinkHover: (key: string | null, rect: DOMRect | null) => void;
  onStartThinkTimer: () => void;
}

function ModelListContent({
  allModels,
  searchQuery,
  selectedModel,
  selectedProvider,
  onSelect,
  thinkHoverKey,
  onThinkHover,
  onStartThinkTimer,
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

  const groupMap = useMemo(() => {
    const groups = new Map<string, FlatModel[]>();
    const source = isSearching ? filteredModels : allModels;
    for (const m of source) {
      const cat = m.providerDisplayName;
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat)!.push(m);
    }
    return groups;
  }, [isSearching, filteredModels, allModels]);

  const sortedGroups = useMemo(() => {
    return Array.from(groupMap.entries())
      .map(([cat, items]) => ({
        category: cat,
        items: items.sort((a, b) => a.name.localeCompare(b.name)),
      }))
      .sort((a, b) => a.category.localeCompare(b.category));
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
      {sortedGroups.map((group) => (
        <div key={group.category}>
          <div className="sticky top-0 z-[1] px-3 py-1 text-[10px] font-semibold text-[var(--text-muted)] uppercase tracking-wider bg-[var(--surface-base)] border-b border-[var(--border-base)]">
            {group.category}
          </div>
          {group.items.map((item) => {
            const key = `${item.provider}/${item.id}`;
            const globalIdx = flatList.indexOf(item);
            const selected = item.id === selectedModel && item.provider === selectedProvider;
            const highlighted = globalIdx === activeIdx;
            const isHovered = thinkHoverKey === key;
            const supportsReasoning = item.reasoning;
            return (
              <div
                key={key}
                ref={(el) => setRowRef(key, el)}
                className="flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs"
                style={{
                  backgroundColor: highlighted
                    ? "var(--surface-hover)"
                    : selected
                      ? "color-mix(in oklab, var(--accent-primary) 12%, var(--surface-base))"
                      : "transparent",
                  color: "var(--text-primary)",
                }}
                onClick={() => onSelect(item)}
                onMouseEnter={() => {
                  setMouseActive(true);
                  setActiveIdx(globalIdx);
                  const el = rowsRef.current.get(key);
                  onThinkHover(key, el ? el.getBoundingClientRect() : null);
                }}
                onMouseLeave={() => {
                  onStartThinkTimer();
                }}
              >
                <span className="truncate">{item.name}</span>
                {selected && (
                  <Check className="w-3 h-3 shrink-0 text-[var(--accent-primary)]" />
                )}
                <ChevronRight className="w-3 h-3 shrink-0 text-[var(--text-muted)] ml-auto" />
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
