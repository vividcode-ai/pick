import { useEffect, useRef, useCallback, type JSX } from "react";
import { Search, X } from "lucide-react";
import { useFilteredList } from "../../hooks/useFilteredList";
import type { GroupInfo } from "../../types/events";

export interface ListSearchProps {
  placeholder?: string;
  autofocus?: boolean;
  action?: JSX.Element;
}

export interface ListProps<T> {
  items: T[];
  key: (item: T) => string;
  filterKeys?: string[];
  current?: T;
  groupBy?: (x: T) => string;
  sortBy?: (a: T, b: T) => number;
  sortGroupsBy?: (a: GroupInfo<T>, b: GroupInfo<T>) => number;
  onSelect?: (value: T | undefined) => void;
  children: (item: T) => JSX.Element;
  emptyMessage?: string;
  search?: ListSearchProps | boolean;
  itemWrapper?: (item: T, node: JSX.Element) => JSX.Element;
  groupHeader?: (group: GroupInfo<T>) => JSX.Element;
  class?: string;
}

let instanceCounter = 0;

export function List<T>(props: ListProps<T>) {
  const listId = useRef(`list-${++instanceCounter}`);
  const scrollRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const {
    filter,
    grouped,
    flat,
    activeIndex,
    mouseActive,
    onInput,
    onKeyDown,
    onMouseEnter,
    setActiveIndex,
  } = useFilteredList<T>({
    items: props.items,
    key: props.key,
    filterKeys: props.filterKeys,
    current: props.current,
    groupBy: props.groupBy,
    sortBy: props.sortBy,
    sortGroupsBy: props.sortGroupsBy,
    onSelect: props.onSelect,
  });

  useEffect(() => {
    if (props.search && typeof props.search === "object" && props.search.autofocus) {
      const timer = setTimeout(() => searchRef.current?.focus(), 80);
      return () => clearTimeout(timer);
    }
  }, [props.search]);

  useEffect(() => {
    const scroll = scrollRef.current;
    if (!scroll || mouseActive || flat.length === 0) return;
    const items = scroll.querySelectorAll<HTMLElement>(`[data-list-key]`);
    const target = items[activeIndex];
    if (!target) return;
    const containerRect = scroll.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    const top = targetRect.top - containerRect.top + scroll.scrollTop;
    const bottom = top + targetRect.height;
    const viewTop = scroll.scrollTop;
    const viewBottom = viewTop + scroll.clientHeight;
    if (top < viewTop) {
      scroll.scrollTop = top;
    } else if (bottom > viewBottom) {
      scroll.scrollTop = bottom - scroll.clientHeight;
    }
  }, [activeIndex, mouseActive, flat.length]);

  const handleClick = useCallback(
    (item: T) => {
      props.onSelect?.(item);
    },
    [props]
  );

  const searchProps = typeof props.search === "object" ? props.search : {};
  const showSearch = !!props.search;

  const hasItems = flat.length > 0;
  const displayEmptyMessage = props.emptyMessage || (filter ? `No results for "${filter}"` : "No items");

  return (
    <div className={`flex flex-col min-h-0 ${props.class ?? ""}`}>
      {showSearch && (
        <div className="flex items-center gap-1 px-2 pt-2 pb-1.5 border-b border-[var(--border-base)]">
          <div className="relative flex-1">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-[var(--text-muted)] pointer-events-none" />
            <input
              ref={searchRef}
              type="text"
              value={filter}
              onChange={(e) => onInput(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder={searchProps.placeholder || "Search..."}
              className="w-full pl-7 pr-7 py-1 text-xs border border-[var(--border-base)] rounded-md bg-[var(--surface-base)] text-[var(--text-primary)] outline-none focus:border-[var(--accent-primary)] focus:shadow-[0_0_0_1px_var(--accent-primary)] placeholder-[var(--text-muted)]"
              spellCheck={false}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
            />
            {filter && (
              <button
                className="absolute right-1 top-1/2 -translate-y-1/2 p-0.5 text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                onClick={() => onInput("")}
                tabIndex={-1}
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>
          {searchProps.action}
        </div>
      )}

      <div ref={scrollRef} className="flex-1 overflow-y-auto py-1 min-h-0 max-h-[260px]" onKeyDown={onKeyDown} tabIndex={-1}>
        {!hasItems ? (
          <div className="px-3 py-6 text-center text-xs text-[var(--text-muted)]">
            {displayEmptyMessage}
          </div>
        ) : (
          grouped.map((group, gi) => (
            <div key={group.category || `group-${gi}`}>
              {group.category && (
                <div className="px-3 py-1 text-[10px] font-semibold text-[var(--text-muted)] uppercase tracking-wider bg-[var(--surface-base)] sticky top-0 z-[1] border-b border-[var(--border-base)]">
                  {props.groupHeader ? props.groupHeader(group) : group.category}
                </div>
              )}
              {group.items.map((item, ii) => {
                const globalIdx = flat.indexOf(item);
                const isSelected = props.current !== undefined && props.key(item) === props.key(props.current!);
                const isHighlighted = globalIdx === activeIndex;
                const node = (
                  <div
                    key={props.key(item)}
                    data-list-key={props.key(item)}
                    className="flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs rounded-none border-none bg-transparent text-left w-full outline-none transition-colors duration-75"
                    style={{
                      backgroundColor: isHighlighted ? "var(--surface-hover)" : isSelected ? "color-mix(in oklab, var(--accent-primary) 12%, var(--surface-base))" : "transparent",
                      color: "var(--text-primary)",
                    }}
                    onClick={() => handleClick(item)}
                    onMouseEnter={() => onMouseEnter(globalIdx)}
                  >
                    {props.children(item)}
                  </div>
                );
                return props.itemWrapper ? props.itemWrapper(item, node) : node;
              })}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
