import { useState, useMemo, useCallback, useRef } from "react";
import fuzzysort from "fuzzysort";
import type { GroupInfo } from "../types/events";

export interface FilteredListProps<T> {
  items: T[];
  key: (item: T) => string;
  filterKeys?: string[];
  current?: T;
  groupBy?: (x: T) => string;
  sortBy?: (a: T, b: T) => number;
  sortGroupsBy?: (a: GroupInfo<T>, b: GroupInfo<T>) => number;
  onSelect?: (value: T | undefined) => void;
}

export function useFilteredList<T>(props: FilteredListProps<T>) {
  const [filter, setFilter] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [mouseActive, setMouseActive] = useState(false);
  const prevFilterRef = useRef(filter);

  const filtered = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (!query) return props.items;
    const needle = query;
    if (!props.filterKeys) {
      return fuzzysort
        .go(needle, props.items as unknown as string[])
        .map((r) => r.target) as unknown as T[];
    }
    const results = fuzzysort.go(needle, props.items, {
      keys: props.filterKeys,
      threshold: -10000,
    });
    return results.map((r) => r.obj);
  }, [props.items, props.filterKeys, filter]);

  const grouped: GroupInfo<T>[] = useMemo(() => {
    const groups = new Map<string, T[]>();
    for (const item of filtered) {
      const category = props.groupBy ? props.groupBy(item) : "";
      if (!groups.has(category)) groups.set(category, []);
      groups.get(category)!.push(item);
    }
    const entries = Array.from(groups.entries()).map(([category, items]) => ({
      category,
      items: props.sortBy ? items.sort(props.sortBy) : items,
    }));
    if (props.sortGroupsBy) {
      entries.sort(props.sortGroupsBy);
    }
    return entries;
  }, [filtered, props.groupBy, props.sortBy, props.sortGroupsBy]);

  const flat: T[] = useMemo(() => {
    return grouped.flatMap((g) => g.items);
  }, [grouped]);

  const resetActive = useCallback(() => {
    setActiveIndex(0);
  }, []);

  const onInput = useCallback(
    (value: string) => {
      const changed = value !== prevFilterRef.current;
      prevFilterRef.current = value;
      setFilter(value);
      if (changed) {
        setActiveIndex(0);
      }
    },
    []
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const len = flat.length;
      if (len === 0) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIndex((i) => (i + 1) % len);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIndex((i) => (i - 1 + len) % len);
      } else if (e.key === "Enter") {
        e.preventDefault();
        const item = flat[activeIndex];
        if (item) props.onSelect?.(item);
      } else if (e.key === "Home") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIndex(0);
      } else if (e.key === "End") {
        e.preventDefault();
        setMouseActive(false);
        setActiveIndex(len - 1);
      }
    },
    [flat, activeIndex, props]
  );

  const onMouseEnter = useCallback(
    (globalIdx: number) => {
      setMouseActive(true);
      setActiveIndex(globalIdx);
    },
    []
  );

  return {
    filter,
    grouped,
    flat,
    activeIndex,
    mouseActive,
    setActiveIndex,
    resetActive,
    onInput,
    onKeyDown,
    onMouseEnter,
    length: flat.length,
  };
}
