// The login mode in Settings ▸ Security (spec `local-auth-bypass`, AC-13 to
// AC-15). Pure so it is unit-tested; SettingsSecurity.svelte renders it.

export type AuthMode = "always" | "private" | "never";

export const AUTH_MODES: readonly AuthMode[] = ["always", "private", "never"];

export interface ServerSettings {
  bind: string;
  auth_mode?: string;
  auth_mode_from_env?: boolean;
}

export interface SecurityView {
  /** The mode in effect. */
  mode: AuthMode;
  /** `KRAKE_AUTH_MODE` sets it; the UI does not offer to change it. */
  fromEnv: boolean;
  /** `never` needs the server to listen on loopback only. */
  neverAllowed: boolean;
}

/** Whether a bind address only accepts connections from this machine. */
export function isLoopbackBind(bind: string): boolean {
  const host = bind.startsWith("[") ? bind.slice(1, bind.indexOf("]")) : bind.replace(/:\d+$/, "");
  return host === "localhost" || host === "::1" || /^127\.\d+\.\d+\.\d+$/.test(host);
}

export function securityView(server: ServerSettings): SecurityView {
  const mode = AUTH_MODES.find((m) => m === server.auth_mode?.trim().toLowerCase()) ?? "always";
  return { mode, fromEnv: server.auth_mode_from_env === true, neverAllowed: isLoopbackBind(server.bind) };
}

/** Switching to a mode that lets requests in without login asks first. */
export function needsConfirmation(from: AuthMode, to: AuthMode): boolean {
  return to !== from && to !== "always";
}
