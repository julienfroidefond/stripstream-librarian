import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { StatusMappingsCard } from "@/app/(app)/settings/components/StatusMappingsCard";

const t = vi.hoisted(() => vi.fn((key: string) => key));
vi.mock("@/lib/i18n/context", () => ({
  useTranslation: () => ({ t, locale: "en", setLocale: vi.fn() }),
}));

const fetchMock = vi.fn();

const initialMappings = [
  { id: "1", provider_status: "ongoing", mapped_status: "reading" },
  { id: "2", provider_status: "hiatus", mapped_status: null },
];

function renderCard(overrides: Partial<Parameters<typeof StatusMappingsCard>[0]> = {}) {
  return render(
    <StatusMappingsCard
      initialStatusMappings={initialMappings}
      initialSeriesStatuses={["reading", "completed"]}
      initialProviderStatuses={["ongoing", "hiatus", "newone"]}
      {...overrides}
    />,
  );
}

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  t.mockImplementation((key: string) => key);
});

describe("StatusMappingsCard", () => {
  it("groups mapped statuses and lists unmapped ones", () => {
    renderCard();
    expect(screen.getByText("ongoing")).toBeInTheDocument();
    expect(screen.getByText("(completed)")).toBeInTheDocument();
    expect(screen.getByText("settings.noMappings")).toBeInTheDocument();
    expect(screen.getByText("hiatus")).toBeInTheDocument();
    expect(screen.getByText("newone")).toBeInTheDocument();
  });

  it("renders the translated status label when available", () => {
    t.mockImplementation((key: string) => (key === "seriesStatus.reading" ? "Lecture" : key));
    renderCard();
    expect(screen.getAllByText("Lecture").length).toBeGreaterThan(0);
  });

  it("assigns an unmapped provider status to a target", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ id: "2", provider_status: "hiatus", mapped_status: "reading" }),
    });
    renderCard();
    const selects = screen.getAllByRole("combobox");
    await userEvent.selectOptions(selects[0], "reading");
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      "/api/settings/status-mappings",
      expect.objectContaining({ method: "POST" }),
    ));
    await waitFor(() => expect(screen.getAllByRole("combobox")).toHaveLength(1));
  });

  it("ignores an empty selection and failed assignment", async () => {
    fetchMock.mockRejectedValue(new Error("boom"));
    renderCard();
    const selects = screen.getAllByRole("combobox") as HTMLSelectElement[];
    await userEvent.selectOptions(selects[0], "reading");
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(screen.getAllByRole("combobox")).toHaveLength(2);
  });

  it("does not update when the assignment response is not ok", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({}) });
    renderCard();
    const selects = screen.getAllByRole("combobox");
    await userEvent.selectOptions(selects[0], "reading");
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(screen.getAllByRole("combobox")).toHaveLength(2);
  });

  it("assigns a brand new provider status", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ id: "3", provider_status: "newone", mapped_status: "completed" }),
    });
    renderCard();
    const selects = screen.getAllByRole("combobox");
    await userEvent.selectOptions(selects[1], "completed");
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      "/api/settings/status-mappings",
      expect.objectContaining({ method: "POST" }),
    ));
    await waitFor(() => expect(screen.getAllByRole("combobox")).toHaveLength(1));
  });

  it("unmaps a provider status", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ id: "1", provider_status: "ongoing", mapped_status: null }),
    });
    renderCard();
    await userEvent.click(screen.getAllByTitle("common.delete")[0]);
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      "/api/settings/status-mappings/1",
      expect.objectContaining({ method: "DELETE" }),
    ));
    await waitFor(() => expect(screen.getAllByRole("combobox")).toHaveLength(3));
  });

  it("tolerates unmap failures", async () => {
    fetchMock.mockRejectedValue(new Error("boom"));
    renderCard();
    await userEvent.click(screen.getAllByTitle("common.delete")[0]);
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(screen.getByText("ongoing")).toBeInTheDocument();
  });

  it("creates a custom target status", async () => {
    renderCard();
    const input = screen.getByPlaceholderText("settings.newTargetPlaceholder");
    await userEvent.type(input, "Action");
    await userEvent.click(screen.getByRole("button", { name: "settings.createTargetStatus" }));
    expect(screen.getByText("(action)")).toBeInTheDocument();
  });

  it("creates a target with the keyboard and rejects duplicates", async () => {
    renderCard();
    const input = screen.getByPlaceholderText("settings.newTargetPlaceholder");
    await userEvent.type(input, "adventure{Enter}");
    expect(screen.getByText("(adventure)")).toBeInTheDocument();

    await userEvent.type(input, "adventure");
    expect(screen.getByRole("button", { name: "settings.createTargetStatus" })).toBeDisabled();
  });

  it("ignores an empty target name", async () => {
    renderCard();
    const submit = screen.getByRole("button", { name: "settings.createTargetStatus" });
    expect(submit).toBeDisabled();
  });
});
