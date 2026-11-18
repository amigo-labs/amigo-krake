// Saving the settings page. The page shows only what the server stored: a
// failed save reloads the stored config instead of keeping the edit, so a
// refused or lost change (say, back to "always") never looks applied
// (spec `local-auth-bypass`, AC-13).

export type SaveResult<T> = { ok: true; config: T } | { ok: false; config: T; error: string };

export async function saveOrRestore<T>(
  edited: T,
  put: (config: T) => Promise<T>,
  get: () => Promise<T>,
): Promise<SaveResult<T>> {
  try {
    return { ok: true, config: await put(edited) };
  } catch (e) {
    const error = e instanceof Error ? e.message : String(e);
    try {
      return { ok: false, config: await get(), error };
    } catch {
      // Not even the stored config is reachable: keep the edit on screen,
      // the error toast says the save failed.
      return { ok: false, config: edited, error };
    }
  }
}
