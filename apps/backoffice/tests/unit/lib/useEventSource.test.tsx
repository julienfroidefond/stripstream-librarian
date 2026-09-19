import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useEventSource } from "@/lib/useEventSource";

class FakeEventSource {
  static instances: FakeEventSource[] = [];

  url: string;
  closed = false;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close() {
    this.closed = true;
  }

  emit(data: string) {
    this.onmessage?.({ data });
  }

  fail() {
    this.onerror?.();
  }
}

function latest() {
  return FakeEventSource.instances[FakeEventSource.instances.length - 1];
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("useEventSource", () => {
  it("opens a connection to the given url", () => {
    renderHook(() => useEventSource({ url: "/api/events", onMessage: vi.fn() }));

    expect(FakeEventSource.instances).toHaveLength(1);
    expect(latest().url).toBe("/api/events");
  });

  it("forwards parsed JSON payloads to onMessage", () => {
    const onMessage = vi.fn();
    renderHook(() => useEventSource({ url: "/api/events", onMessage }));

    act(() => latest().emit(JSON.stringify({ count: 3 })));

    expect(onMessage).toHaveBeenCalledWith({ count: 3 });
  });

  it("ignores malformed payloads", () => {
    const onMessage = vi.fn();
    renderHook(() => useEventSource({ url: "/api/events", onMessage }));

    act(() => latest().emit("{"));

    expect(onMessage).not.toHaveBeenCalled();
  });

  it("reconnects after a connection error", () => {
    renderHook(() => useEventSource({ url: "/api/events", onMessage: vi.fn() }));
    const first = latest();

    act(() => first.fail());
    expect(first.closed).toBe(true);

    act(() => vi.advanceTimersByTime(3000));
    expect(FakeEventSource.instances).toHaveLength(2);
  });

  it("reconnects when no message arrives before the stale timeout", () => {
    renderHook(() =>
      useEventSource({ url: "/api/events", onMessage: vi.fn(), staleTimeoutMs: 1000 })
    );
    const first = latest();

    act(() => vi.advanceTimersByTime(1000));

    expect(first.closed).toBe(true);
    expect(FakeEventSource.instances).toHaveLength(2);
  });

  it("resets the stale timer whenever a message is received", () => {
    renderHook(() =>
      useEventSource({ url: "/api/events", onMessage: vi.fn(), staleTimeoutMs: 1000 })
    );

    act(() => vi.advanceTimersByTime(800));
    act(() => latest().emit("{}"));
    act(() => vi.advanceTimersByTime(800));

    expect(FakeEventSource.instances).toHaveLength(1);
  });

  it("disconnects when the tab is hidden and reconnects when visible again", () => {
    renderHook(() => useEventSource({ url: "/api/events", onMessage: vi.fn() }));
    const first = latest();

    Object.defineProperty(document, "hidden", { configurable: true, value: true });
    act(() => document.dispatchEvent(new Event("visibilitychange")));
    expect(first.closed).toBe(true);

    Object.defineProperty(document, "hidden", { configurable: true, value: false });
    act(() => document.dispatchEvent(new Event("visibilitychange")));
    expect(FakeEventSource.instances).toHaveLength(2);
  });

  it("always calls the latest onMessage without reconnecting", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ onMessage }) => useEventSource({ url: "/api/events", onMessage }),
      { initialProps: { onMessage: first } }
    );

    rerender({ onMessage: second });
    act(() => latest().emit(JSON.stringify({ n: 1 })));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith({ n: 1 });
    expect(FakeEventSource.instances).toHaveLength(1);
  });

  it("closes the connection on unmount", () => {
    const { unmount } = renderHook(() =>
      useEventSource({ url: "/api/events", onMessage: vi.fn() })
    );
    const source = latest();

    unmount();

    expect(source.closed).toBe(true);
  });
});
