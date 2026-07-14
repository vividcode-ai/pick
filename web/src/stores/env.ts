import { useSyncExternalStore } from "react";

export type EnvType = "tauri" | "web";

let envType: EnvType = "web";
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

export function setEnvType(type: EnvType) {
  envType = type;
  emit();
}

export function getEnvType(): EnvType {
  return envType;
}

export function subscribeToEnv(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

export function useEnvType(): EnvType {
  return useSyncExternalStore(subscribeToEnv, getEnvType, getEnvType);
}
