import { describe, it, expect } from "vitest";
import { saveOrRestore } from "../src/lib/settings";

// Settings show only what the server stored (spec local-auth-bypass, AC-13):
// a failed save restores the server's config instead of keeping the edit.
const stored = { server: { bind: "0.0.0.0:1516", auth_mode: "private" } };

describe("saveOrRestore", () => {
  it("returns the config the server stored", async () => {
    const saved = { server: { bind: "0.0.0.0:1516", auth_mode: "always" } };
    const result = await saveOrRestore(
      { server: { bind: "0.0.0.0:1516", auth_mode: "always" } },
      async () => saved,
      async () => stored,
    );
    expect(result).toEqual({ ok: true, config: saved });
  });

  it("reloads the stored config when the save fails", async () => {
    const edited = { server: { bind: "0.0.0.0:1516", auth_mode: "always" } };
    const result = await saveOrRestore(
      edited,
      async () => {
        throw new Error("server.auth_mode \"never\" needs a loopback bind");
      },
      async () => stored,
    );
    expect(result).toEqual({
      ok: false,
      config: stored,
      error: 'server.auth_mode "never" needs a loopback bind',
    });
  });

  it("keeps the edit only when even the reload fails", async () => {
    const edited = { server: { bind: "x", auth_mode: "always" } };
    const fail = async () => {
      throw new Error("offline");
    };
    const result = await saveOrRestore(edited, fail, fail);
    expect(result.ok).toBe(false);
    expect(result.config).toBe(edited);
  });
});
