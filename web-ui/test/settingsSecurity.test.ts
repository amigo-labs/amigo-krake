import { describe, it, expect } from "vitest";
import { render } from "svelte/server";
import SettingsSecurity from "../src/components/settings/SettingsSecurity.svelte";
import { chooseMode, securityView } from "../src/lib/security";
import { locale, translations } from "../src/lib/i18n";

// Settings ▸ Security rendered to HTML (spec local-auth-bypass, AC-09,
// AC-13, AC-14), and the decision behind a click (AC-15).

function html(server: Record<string, unknown>, lang: "en" | "de" = "en"): string {
  locale.set(lang);
  const config = { server } as never;
  return render(SettingsSecurity, { props: { config, onsave: () => {} } }).body;
}

const text = (s: string) => s.replace(/<[^>]+>/g, " ").replace(/&amp;/g, "&").replace(/\s+/g, " ");

describe("Settings ▸ Security (AC-13)", () => {
  it("shows the three modes with explanations, the current mode and the warning", () => {
    const out = html({ bind: "0.0.0.0:1516", auth_mode: "private" });
    const t = text(out);
    for (const key of ["always", "private", "never"]) {
      expect(t).toContain(translations.en[`security.${key}`]);
      expect(t).toContain(translations.en[`security.${key}_desc`]);
    }
    expect(t).toContain("full control");
    for (const word of ["Docker Desktop", "rootless Docker", "Podman", "tunnel"]) expect(t).toContain(word);
    expect(out).toMatch(/value="private"[^>]*checked|checked[^>]*value="private"/);
    expect(out).not.toMatch(/value="always"[^>]*checked|checked[^>]*value="always"/);
  });

  it("renders in German too (AC-16)", () => {
    expect(text(html({ bind: "0.0.0.0:1516" }, "de"))).toContain("Wer sich anmelden muss");
    locale.set("en");
  });
});

describe("never needs a loopback bind (AC-14)", () => {
  it("is disabled with the reason on a reachable bind", () => {
    const out = html({ bind: "0.0.0.0:1516", auth_mode: "always" });
    expect(out).toMatch(/value="never"[^>]*disabled|disabled[^>]*value="never"/);
    expect(text(out)).toContain("Krake listens on 0.0.0.0:1516");
  });

  it("is offered on a loopback bind", () => {
    const out = html({ bind: "127.0.0.1:1516", auth_mode: "always" });
    expect(out).not.toMatch(/value="never"[^>]*disabled|disabled[^>]*value="never"/);
  });
});

describe("a mode set by the environment is locked (AC-09)", () => {
  it("disables the choice and says why", () => {
    const out = html({ bind: "0.0.0.0:1516", auth_mode: "private", auth_mode_from_env: true });
    expect(out).toMatch(/<fieldset[^>]*disabled/);
    expect(text(out)).toContain("KRAKE_AUTH_MODE");
  });
});

describe("chooseMode (AC-15)", () => {
  const local = securityView({ bind: "127.0.0.1:1516", auth_mode: "always" });

  it("asks before turning the login off and applies only on confirm", () => {
    expect(chooseMode(local, "private")).toEqual({ kind: "confirm", mode: "private" });
    expect(chooseMode(local, "never")).toEqual({ kind: "confirm", mode: "never" });
  });

  it("applies turning the login on directly", () => {
    const priv = securityView({ bind: "127.0.0.1:1516", auth_mode: "private" });
    expect(chooseMode(priv, "always")).toEqual({ kind: "apply", mode: "always" });
  });

  it("does nothing for the current mode, a locked mode or never on a reachable bind", () => {
    expect(chooseMode(local, "always")).toEqual({ kind: "none" });
    const env = securityView({ bind: "127.0.0.1:1516", auth_mode: "always", auth_mode_from_env: true });
    expect(chooseMode(env, "private")).toEqual({ kind: "none" });
    const lan = securityView({ bind: "0.0.0.0:1516", auth_mode: "always" });
    expect(chooseMode(lan, "never")).toEqual({ kind: "none" });
  });
});
