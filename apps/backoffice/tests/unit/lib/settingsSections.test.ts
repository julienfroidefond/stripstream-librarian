import { describe, expect, it } from "vitest";

import {
  DEFAULT_TELEGRAM_EVENTS,
  SETTINGS_SECTION_DEFAULTS,
  seedSettingsSections,
  validateSettingsSection,
} from "@/lib/settingsSections";

describe("seedSettingsSections", () => {
  it("fills missing sections with their defaults", () => {
    const seeded = seedSettingsSections({});

    expect(seeded.prowlarr).toEqual(SETTINGS_SECTION_DEFAULTS.prowlarr);
    expect(seeded.downloads_enabled).toEqual({ enabled: false });
    expect(seeded.rename_format).toBe("{series_name} - T{volume_padded} - {title}");
  });

  it("merges stored fields over defaults and fills nested defaults", () => {
    const seeded = seedSettingsSections({
      telegram: { enabled: true, chat_id: "42" },
      limits: { concurrent_renders: 8 },
    });

    expect(seeded.telegram).toMatchObject({
      enabled: true,
      chat_id: "42",
      bot_token: "",
      events: DEFAULT_TELEGRAM_EVENTS,
    });
    expect(seeded.limits).toMatchObject({ concurrent_renders: 8, timeout_seconds: 12 });
  });

  it("replaces arrays instead of merging them", () => {
    const seeded = seedSettingsSections({ prowlarr: { categories: [1234] } });

    expect((seeded.prowlarr as { categories: number[] }).categories).toEqual([1234]);
  });

  it("keeps unknown stored sections and ignores nullish ones", () => {
    const seeded = seedSettingsSections({
      custom_section: { anything: true },
      prowlarr: null,
    });

    expect(seeded.custom_section).toEqual({ anything: true });
    expect(seeded.prowlarr).toEqual(SETTINGS_SECTION_DEFAULTS.prowlarr);
  });

  it("falls back to defaults for nullish nested fields", () => {
    const seeded = seedSettingsSections({
      telegram: { events: null, chat_id: null, bot_token: undefined },
      limits: { concurrent_renders: null },
    });

    expect(seeded.telegram).toMatchObject({
      chat_id: "",
      bot_token: "",
      events: DEFAULT_TELEGRAM_EVENTS,
    });
    expect((seeded.limits as { concurrent_renders: number }).concurrent_renders).toBe(4);
  });

  it("does not mutate the defaults", () => {
    const seeded = seedSettingsSections({ prowlarr: { url: "http://prowlarr:9696" } });
    (seeded.prowlarr as { url: string }).url = "changed";

    expect((SETTINGS_SECTION_DEFAULTS.prowlarr as { url: string }).url).toBe("");
  });
});

describe("validateSettingsSection", () => {
  it("rejects enabling an integration without its required fields", () => {
    expect(validateSettingsSection("prowlarr", { enabled: true, url: "", api_key: "" }, { enabled: false }))
      .toBe("settings.validationProwlarr");
    expect(validateSettingsSection("qbittorrent", { enabled: true, url: "http://qb", username: "" }, { enabled: false }))
      .toBe("settings.validationQbittorrent");
    expect(validateSettingsSection("alldebrid", { enabled: true, api_key: "" }, { enabled: false }))
      .toBe("settings.validationAlldebrid");
    expect(validateSettingsSection("telegram", { enabled: true, bot_token: "token", chat_id: "" }, { enabled: false }))
      .toBe("settings.validationTelegram");
    expect(validateSettingsSection("ai_tagging", { enabled: true, base_url: "", model: "free" }, { enabled: false }))
      .toBe("settings.validationAi");
  });

  it("lets an already-enabled integration be filled in field by field", () => {
    expect(validateSettingsSection("prowlarr", { enabled: true, url: "http://prowlarr:9696", api_key: "" }, { enabled: true }))
      .toBeNull();
    expect(validateSettingsSection("qbittorrent", { enabled: true, url: "http://qb", username: "" }, { enabled: true }))
      .toBeNull();
    expect(validateSettingsSection("telegram", { enabled: true, bot_token: "", chat_id: "" }, { enabled: true }))
      .toBeNull();
  });

  it("allows disabling any integration", () => {
    expect(validateSettingsSection("prowlarr", { enabled: false }, { enabled: true })).toBeNull();
    expect(validateSettingsSection("qbittorrent", { enabled: false }, { enabled: true })).toBeNull();
    expect(validateSettingsSection("alldebrid", { enabled: false }, { enabled: true })).toBeNull();
    expect(validateSettingsSection("telegram", { enabled: false }, { enabled: true })).toBeNull();
    expect(validateSettingsSection("ai_tagging", { enabled: false }, { enabled: true })).toBeNull();
  });

  it("accepts a configured integration", () => {
    expect(validateSettingsSection("prowlarr", {
      enabled: true,
      url: " http://prowlarr:9696 ",
      api_key: " secret ",
    }, { enabled: false })).toBeNull();
    expect(validateSettingsSection("alldebrid", {
      enabled: true,
      // The API returns the redacted placeholder once a key is stored.
      api_key: "********",
    }, { enabled: false })).toBeNull();
  });

  it("requires a non-empty cache and thumbnail directory", () => {
    expect(validateSettingsSection("cache", { directory: "  " }))
      .toBe("settings.validationCacheDirectory");
    expect(validateSettingsSection("thumbnail", { directory: "" }))
      .toBe("settings.validationThumbnailDirectory");
    expect(validateSettingsSection("cache", { directory: "/tmp/cache" })).toBeNull();
  });

  it("only accepts a known download client", () => {
    expect(validateSettingsSection("download_client", { client: "qbittorrent" })).toBeNull();
    expect(validateSettingsSection("download_client", { client: "alldebrid" })).toBeNull();
    expect(validateSettingsSection("download_client", { client: "sabnzbd" }))
      .toBe("settings.validationDownloadClient");
    expect(validateSettingsSection("download_client", {})).toBeNull();
  });

  it("leaves unrelated sections alone", () => {
    expect(validateSettingsSection("limits", null)).toBeNull();
    expect(validateSettingsSection("rename_format", "")).toBeNull();
  });
});
