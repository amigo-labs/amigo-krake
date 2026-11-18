// Which page the app opens with: setup, login or the app itself. A server
// that lets the browser in without a login answers `/me`, so the app opens
// directly (spec `local-auth-bypass`, AC-12).

export type BootState = "loading" | "setup" | "login" | "ready";

export async function detectBootState(
  getSetupStatus: () => Promise<{ needs_setup: boolean }>,
  me: () => Promise<unknown>,
): Promise<BootState> {
  try {
    const s = await getSetupStatus();
    if (s.needs_setup) return "setup";
  } catch {
    // /setup/status is public; a failure here means the server is
    // unreachable — show the login form so the user sees *something*.
    return "login";
  }
  try {
    await me();
    return "ready";
  } catch {
    return "login";
  }
}
