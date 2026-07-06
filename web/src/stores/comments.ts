import type { LineComment } from "../types/events";

const STORAGE_KEY = "pick_comments";

function load(): LineComment[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function save(comments: LineComment[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(comments));
  } catch {}
}

let comments = load();
let listeners: Set<() => void> = new Set();

function emit() {
  listeners.forEach((l) => l());
}

export function subscribeToComments(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

export function getCommentsSnapshot() {
  return comments;
}

export function getCommentsByFile(file: string): LineComment[] {
  return comments.filter((c) => c.file === file);
}

export function addComment(input: Omit<LineComment, "id" | "time">): LineComment {
  const next: LineComment = {
    id: crypto.randomUUID(),
    time: Date.now(),
    ...input,
  };
  comments = [...comments, next];
  save(comments);
  emit();
  return next;
}

export function removeComment(id: string) {
  comments = comments.filter((c) => c.id !== id);
  save(comments);
  emit();
}

export function updateComment(id: string, updater: Partial<Omit<LineComment, "id">>) {
  comments = comments.map((c) => (c.id === id ? { ...c, ...updater } : c));
  save(comments);
  emit();
}

export function resolveComment(id: string) {
  updateComment(id, { resolved: true });
}

export function unresolveComment(id: string) {
  updateComment(id, { resolved: false });
}

export function getAllComments(): LineComment[] {
  return comments;
}

export function syncCommentsFromServer(serverComments: LineComment[]) {
  comments = serverComments;
  save(comments);
  emit();
}

export async function loadCommentsFromServer(baseUrl: string, sessionId: string) {
  try {
    const res = await fetch(`${baseUrl}/comments/${sessionId}`);
    if (res.ok) {
      const data = await res.json();
      syncCommentsFromServer(data.comments || []);
    }
  } catch (e) {
    console.error("Failed to load comments from server:", e);
  }
}

export async function saveCommentsToServer(baseUrl: string, sessionId: string) {
  try {
    await fetch(`${baseUrl}/comments/${sessionId}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ comments }),
    });
  } catch (e) {
    console.error("Failed to save comments to server:", e);
  }
}
