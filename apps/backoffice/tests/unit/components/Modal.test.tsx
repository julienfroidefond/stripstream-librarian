import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Modal } from "@/app/components/ui/Modal";

describe("Modal", () => {
  it("renders nothing when closed", () => {
    render(
      <Modal isOpen={false} onClose={vi.fn()}>
        <p>body</p>
      </Modal>
    );

    expect(screen.queryByText("body")).not.toBeInTheDocument();
    expect(screen.queryByTestId("modal-panel")).not.toBeInTheDocument();
  });

  it("renders title, body and footer when open", () => {
    render(
      <Modal isOpen onClose={vi.fn()} title="My title" footer={<button type="button">save</button>}>
        <p>body</p>
      </Modal>
    );

    expect(screen.getByText("My title")).toBeInTheDocument();
    expect(screen.getByText("body")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "save" })).toBeInTheDocument();
  });

  it("omits the header and footer when they are not provided", () => {
    render(
      <Modal isOpen onClose={vi.fn()}>
        <p>body</p>
      </Modal>
    );

    expect(screen.queryByTestId("modal-close")).not.toBeInTheDocument();
  });

  it("closes via the close button", () => {
    const onClose = vi.fn();
    render(
      <Modal isOpen onClose={onClose} title="My title">
        <p>body</p>
      </Modal>
    );

    fireEvent.click(screen.getByTestId("modal-close"));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape", () => {
    const onClose = vi.fn();
    render(
      <Modal isOpen onClose={onClose} title="My title">
        <p>body</p>
      </Modal>
    );

    fireEvent.keyDown(document, { key: "Escape" });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes when clicking the backdrop but not the panel", () => {
    const onClose = vi.fn();
    render(
      <Modal isOpen onClose={onClose} title="My title">
        <p>body</p>
      </Modal>
    );

    fireEvent.click(screen.getByTestId("modal-panel"));
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("modal-panel").parentElement!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("blocks every close path when disableClose is set", () => {
    const onClose = vi.fn();
    render(
      <Modal isOpen onClose={onClose} title="My title" disableClose>
        <p>body</p>
      </Modal>
    );

    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(screen.getByTestId("modal-panel").parentElement!);

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByTestId("modal-close")).toBeDisabled();
  });

  it("applies the requested max width", () => {
    render(
      <Modal isOpen onClose={vi.fn()} maxWidth="4xl">
        <p>body</p>
      </Modal>
    );

    expect(screen.getByTestId("modal-panel")).toHaveClass("max-w-4xl");
  });

  it("moves focus into the panel and traps Tab navigation", () => {
    const trigger = document.createElement("button");
    trigger.textContent = "open";
    document.body.append(trigger);
    trigger.focus();

    try {
      render(
        <Modal isOpen onClose={vi.fn()} title="My title">
          <button type="button">first</button>
          <button type="button">last</button>
        </Modal>
      );

      const close = screen.getByTestId("modal-close");
      const last = screen.getByRole("button", { name: "last" });
      expect(close).toHaveFocus();

      last.focus();
      fireEvent.keyDown(document, { key: "Tab" });
      expect(close).toHaveFocus();

      close.focus();
      fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
      expect(last).toHaveFocus();
    } finally {
      trigger.remove();
    }
  });

  it("restores focus to the trigger when closed", () => {
    const trigger = document.createElement("button");
    trigger.textContent = "open";
    document.body.append(trigger);
    trigger.focus();

    try {
      const { rerender } = render(
        <Modal isOpen onClose={vi.fn()} title="My title">
          <p>body</p>
        </Modal>
      );
      expect(screen.getByTestId("modal-close")).toHaveFocus();

      rerender(
        <Modal isOpen={false} onClose={vi.fn()} title="My title">
          <p>body</p>
        </Modal>
      );
      expect(trigger).toHaveFocus();
    } finally {
      trigger.remove();
    }
  });

  it("restores the trigger even when an autoFocus field moves focus on open", () => {
    const trigger = document.createElement("button");
    trigger.textContent = "open";
    document.body.append(trigger);

    try {
      const { rerender } = render(
        <Modal isOpen={false} onClose={vi.fn()} title="My title">
          <input aria-label="search" autoFocus />
        </Modal>
      );
      trigger.focus();
      fireEvent.pointerDown(trigger);

      rerender(
        <Modal isOpen onClose={vi.fn()} title="My title">
          <input aria-label="search" autoFocus />
        </Modal>
      );
      expect(screen.getByRole("textbox", { name: "search" })).toHaveFocus();

      rerender(
        <Modal isOpen={false} onClose={vi.fn()} title="My title">
          <input aria-label="search" autoFocus />
        </Modal>
      );
      expect(trigger).toHaveFocus();
    } finally {
      trigger.remove();
    }
  });
});
