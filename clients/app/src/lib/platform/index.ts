import { isTauri as tauriRuntimeDetected } from "@tauri-apps/api/core";

/** True when the UI runs inside the Tauri shell (desktop, iOS, Android). */
export function isTauri(): boolean {
  return tauriRuntimeDetected();
}
