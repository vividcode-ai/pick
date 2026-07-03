import { useState, useCallback, useSyncExternalStore } from "react";

export interface SessionEntry {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  archived?: boolean;
}

let sessions: SessionEntry[] = [];
let listeners: Set<() => void> = new Set();

function subscribeToSessions(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

function getSnapshot() {
  return sessions;
}

function emitChange() {
  listeners.forEach((l) => l());
}

export function initSessions(list: SessionEntry[]) {
  sessions = list;
  emitChange();
}

export function addSessionEntry(id: string, title?: string) {
  const now = Date.now();
  sessions = [
    { id, title: title || `Session ${sessions.length + 1}`, createdAt: now, updatedAt: now },
    ...sessions,
  ];
  emitChange();
}

export function removeSessionEntry(id: string) {
  sessions = sessions.filter((s) => s.id !== id);
  emitChange();
}

export function renameSessionEntry(id: string, title: string) {
  sessions = sessions.map((s) => (s.id === id ? { ...s, title, updatedAt: Date.now() } : s));
  emitChange();
}

export function archiveSessionEntry(id: string) {
  sessions = sessions.map((s) => (s.id === id ? { ...s, archived: true } : s));
  emitChange();
}

export function unarchiveSessionEntry(id: string) {
  sessions = sessions.map((s) => (s.id === id ? { ...s, archived: false } : s));
  emitChange();
}

export function useSessionList() {
  const list = useSyncExternalStore(subscribeToSessions, getSnapshot, getSnapshot);
  return list;
}

export function useSessionSearch() {
  const [query, setQuery] = useState("");
  const list = useSessionList();

  const activeList = list.filter((s) => !s.archived);

  const filtered = query.trim()
    ? activeList.filter((s) => s.title.toLowerCase().includes(query.toLowerCase()))
    : activeList;

  return { query, setQuery, filtered };
}

export function useArchivedSessions() {
  const list = useSessionList();
  return list.filter((s) => s.archived);
}
