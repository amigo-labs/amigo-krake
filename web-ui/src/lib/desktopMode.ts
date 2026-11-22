// The desktop app's update-choice state, shared by the first-start question,
// the announcement and Settings ▸ About (spec `desktop-app`, AC-34). `null`
// means "not the desktop app": `GET /updates/mode` answered 404.

import { writable } from "svelte/store";
import { ApiError, getUpdateMode, type DesktopModeState } from "./api";

export const desktopMode = writable<DesktopModeState | null>(null);

/** True once `GET /updates/mode` answered at all (200 or 404). */
export const desktopModeKnown = writable(false);

/**
 * Reload the state. A 404 means no desktop mode; any other failure (offline
 * server, 401) keeps what was known, so a hiccup does not hide the section.
 */
export async function refreshDesktopMode(): Promise<DesktopModeState | null> {
  try {
    const state = await getUpdateMode();
    desktopMode.set(state);
    desktopModeKnown.set(true);
    return state;
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) {
      desktopMode.set(null);
      desktopModeKnown.set(true);
    }
    return null;
  }
}
