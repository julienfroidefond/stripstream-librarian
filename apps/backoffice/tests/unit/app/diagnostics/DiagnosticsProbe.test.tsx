import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/app/components/ui")>();
  return { ...actual, toast: vi.fn() };
});

import { toast } from "@/app/components/ui";

import { DiagnosticsProbePanel } from "@/app/(app)/diagnostics/DiagnosticsProbe";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
});

function ok(payload: unknown) {
  return { ok: true, status: 200, json: async () => payload };
}

const PROBE = {
  sampled_at: "2026-01-02T03:04:05Z",
  integrations: [
    { id: "qbittorrent", configured: true, reachable: true, latency_ms: 12, detail: "5.0.2" },
    { id: "prowlarr", configured: true, reachable: false, latency_ms: 5000, detail: "connection failed" },
    { id: "telegram", configured: false, reachable: false, latency_ms: 0, detail: null },
    { id: "ddl_fourtoutici", configured: true, reachable: true, latency_ms: 80, detail: null },
  ],
  missing_files: { checked: 1200, missing: 2, samples: ["/libraries/gon/01.cbz", "/libraries/gon/02.cbz"] },
};

describe("DiagnosticsProbePanel", () => {
  it("explains that no probe ran yet", () => {
    render(<DiagnosticsProbePanel />);

    expect(screen.getByText("diagnostics.probeNotRun")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /diagnostics.runProbe/ })).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("runs the probe and renders integrations and missing files", async () => {
    fetchMock.mockResolvedValue(ok(PROBE));
    render(<DiagnosticsProbePanel />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.runProbe/ }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/diagnostics/probe"));

    expect(await screen.findByText("qBittorrent")).toBeInTheDocument();
    expect(screen.getByText("Prowlarr")).toBeInTheDocument();
    expect(screen.getByText("Telegram")).toBeInTheDocument();
    expect(screen.getByText("DDL · Fourtoutici")).toBeInTheDocument();

    expect(screen.getAllByText("diagnostics.probeOk")).toHaveLength(2);
    expect(screen.getAllByText("diagnostics.probeKo")).toHaveLength(1);
    expect(screen.getAllByText("diagnostics.probeNotConfigured")).toHaveLength(1);
    expect(screen.getByText("connection failed")).toBeInTheDocument();

    expect(screen.getByText("1,200")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("/libraries/gon/01.cbz")).toBeInTheDocument();
  });

  it("celebrates when every checked file is present", async () => {
    fetchMock.mockResolvedValue(ok({ ...PROBE, missing_files: { checked: 5, missing: 0, samples: [] } }));
    render(<DiagnosticsProbePanel />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.runProbe/ }));

    expect(await screen.findByText("diagnostics.probeNoMissing")).toBeInTheDocument();
    expect(screen.queryByText("diagnostics.probeSamples")).not.toBeInTheDocument();
  });

  it("reports a probe failure", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 500, json: async () => ({}) });
    render(<DiagnosticsProbePanel />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.runProbe/ }));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("diagnostics.probeError", "error"));
    expect(screen.getByText("diagnostics.probeNotRun")).toBeInTheDocument();
  });
});
