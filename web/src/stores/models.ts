interface FavoriteKey {
  providerId: string;
  modelId: string;
}

const STORAGE_KEY = "pick_model_favorites";

function loadFavorites(): FavoriteKey[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function saveFavorites(favorites: FavoriteKey[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(favorites));
}

let favorites = loadFavorites();
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

export function isFavorite(providerId: string, modelId: string): boolean {
  return favorites.some((f) => f.providerId === providerId && f.modelId === modelId);
}

export function toggleFavorite(providerId: string, modelId: string) {
  const idx = favorites.findIndex(
    (f) => f.providerId === providerId && f.modelId === modelId
  );
  if (idx >= 0) {
    favorites = [...favorites.slice(0, idx), ...favorites.slice(idx + 1)];
  } else {
    favorites = [...favorites, { providerId, modelId }];
  }
  saveFavorites(favorites);
  emit();
}

export function getFavoriteModelKeys(): Set<string> {
  return new Set(favorites.map((f) => `${f.providerId}/${f.modelId}`));
}

export function subscribeToFavorites(callback: () => void) {
  listeners.add(callback);
  return () => { listeners.delete(callback); };
}

export function getFavoritesSnapshot(): number {
  return favorites.length;
}
