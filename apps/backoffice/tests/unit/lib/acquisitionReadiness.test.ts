import { describe, expect, it } from "vitest";

import {
  EMPTY_ACQUISITION_READINESS,
  isProwlarrConfigured,
  resolveAcquisitionReadiness,
  type AcquisitionReadinessSnapshot,
} from "@/lib/acquisitionReadiness";

function settings(overrides: Record<string, unknown> = {}) {
  return {
    downloads_enabled: { enabled: true },
    prowlarr: { enabled: true, url: "http://prowlarr:9696", api_key: "secret" },
    qbittorrent: { enabled: true, url: "http://qbittorrent:8080", username: "admin" },
    alldebrid: { enabled: true, api_key: "redacted-but-non-empty" },
    ...overrides,
  };
}

function resolve(
  settingsOverrides: Record<string, unknown> = {},
  extra: Omit<AcquisitionReadinessSnapshot, "settings"> = {}
) {
  return resolveAcquisitionReadiness({
    settings: settings(settingsOverrides),
    telegram: { configured: true, authorized: true },
    ddlProviders: [{ enabled: true }],
    ...extra,
  });
}

describe("resolveAcquisitionReadiness", () => {
  it("exposes every source when the master flag and each source rule pass", () => {
    expect(resolve()).toEqual({
      downloads: true,
      prowlarr: true,
      qbittorrent: true,
      alldebrid: true,
      telegram: true,
      ddl: true,
    });
  });

  it("turns every source off when the downloads master flag is disabled", () => {
    expect(resolve({ downloads_enabled: { enabled: false } })).toEqual(
      EMPTY_ACQUISITION_READINESS
    );
  });

  it("turns every source off when the settings snapshot is missing", () => {
    expect(
      resolveAcquisitionReadiness({
        settings: null,
        telegram: { configured: true, authorized: true },
        ddlProviders: [{ enabled: true }],
      })
    ).toEqual(EMPTY_ACQUISITION_READINESS);
  });

  it("requires a complete and non-disabled Prowlarr configuration", () => {
    expect(resolve({ prowlarr: { enabled: false, url: "http://p", api_key: "k" } }).prowlarr).toBe(false);
    expect(resolve({ prowlarr: { enabled: true, url: "", api_key: "k" } }).prowlarr).toBe(false);
    expect(resolve({ prowlarr: { enabled: true, url: "http://p", api_key: "   " } }).prowlarr).toBe(false);
    expect(resolve({ prowlarr: { url: "http://p", api_key: "k" } }).prowlarr).toBe(true);
  });

  it("reuses the shared download-client rules", () => {
    expect(resolve({ qbittorrent: { enabled: false, url: "http://q", username: "u" } }).qbittorrent).toBe(false);
    expect(resolve({ qbittorrent: { url: "http://q", username: "" } }).qbittorrent).toBe(false);
    expect(resolve({ alldebrid: { enabled: false, api_key: "k" } }).alldebrid).toBe(false);
    expect(resolve({ alldebrid: { enabled: true, api_key: "" } }).alldebrid).toBe(false);
  });

  it("requires Telegram to be both configured and authorized", () => {
    expect(resolve({}, { telegram: { configured: true, authorized: false } }).telegram).toBe(false);
    expect(resolve({}, { telegram: { configured: false, authorized: true } }).telegram).toBe(false);
    expect(resolve({}, { telegram: null }).telegram).toBe(false);
  });

  it("requires at least one enabled DDL provider", () => {
    expect(resolve({}, { ddlProviders: [] }).ddl).toBe(false);
    expect(resolve({}, { ddlProviders: [{ enabled: false }] }).ddl).toBe(false);
    expect(resolve({}, { ddlProviders: null }).ddl).toBe(false);
  });
});

describe("isProwlarrConfigured", () => {
  it("accepts a full configuration and rejects missing credentials", () => {
    expect(isProwlarrConfigured({ url: "http://p", api_key: "k" })).toBe(true);
    expect(isProwlarrConfigured({ url: "http://p", api_key: "" })).toBe(false);
    expect(isProwlarrConfigured(null)).toBe(false);
  });
});
