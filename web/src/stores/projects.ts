import { useSyncExternalStore } from "react";

export interface ProjectEntry {
  path: string;
  name: string;
  last_used_at?: number;
}

interface ProjectState {
  projects: ProjectEntry[];
  currentCwd: string | null;
  loading: boolean;
  error: string | null;
}

let state: ProjectState = {
  projects: [],
  currentCwd: null,
  loading: false,
  error: null,
};
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

function getSnapshot(): ProjectState {
  return state;
}

/** Synchronous accessor — safe to use outside React hooks. */
export function getCurrentCwd(): string | null {
  return state.currentCwd;
}

export function subscribeToProjects(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

export function useProjectsState() {
  return useSyncExternalStore(subscribeToProjects, getSnapshot, getSnapshot);
}

export async function fetchProjects(baseUrl: string) {
  state = { ...state, loading: true };
  emit();
  try {
    const res = await fetch(`${baseUrl}/cwd`);
    const cwdData = res.ok ? await res.json() : null;
    const projRes = await fetch(`${baseUrl}/projects`);
    const data = projRes.ok ? await projRes.json() : { projects: [], current_cwd: null };
    state = {
      projects: data.projects || [],
      currentCwd: data.current_cwd || (cwdData?.cwd || null),
      loading: false,
      error: null,
    };
  } catch (e) {
    state = { ...state, loading: false, error: String(e) };
  }
  emit();
}

export async function switchProject(baseUrl: string, path: string, loadSessions?: boolean): Promise<boolean> {
  try {
    const res = await fetch(`${baseUrl}/cwd`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ cwd: path, load_sessions: loadSessions ?? false }),
    });
    if (!res.ok) return false;
    const data = await res.json();
    state = { ...state, currentCwd: data.cwd };
    emit();
    return true;
  } catch {
    return false;
  }
}

export async function addProject(baseUrl: string, path: string): Promise<boolean> {
  try {
    const res = await fetch(`${baseUrl}/projects`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ cwd: path }),
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function removeProject(baseUrl: string, path: string): Promise<boolean> {
  try {
    const res = await fetch(`${baseUrl}/projects/remove`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ cwd: path }),
    });
    if (!res.ok) return false;
    // Remove from local state immediately
    state = { ...state, projects: state.projects.filter((p) => p.path !== path) };
    emit();
    return true;
  } catch {
    return false;
  }
}
