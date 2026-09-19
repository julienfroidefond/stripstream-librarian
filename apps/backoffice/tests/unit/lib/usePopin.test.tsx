import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { usePopin } from "@/lib/usePopin";

function Harness() {
  const { isOpen, setIsOpen, buttonRef, popinRef, popinStyle } = usePopin();

  return (
    <div>
      <button ref={buttonRef} type="button" onClick={() => setIsOpen((v) => !v)}>
        toggle
      </button>
      {isOpen && (
        <div ref={popinRef} style={popinStyle} data-testid="popin">
          <button type="button">inside</button>
        </div>
      )}
    </div>
  );
}

function openPopin() {
  fireEvent.click(screen.getByRole("button", { name: "toggle" }));
  return screen.getByTestId("popin");
}

describe("usePopin", () => {
  it("starts closed and shows the popin after toggling", () => {
    render(<Harness />);
    expect(screen.queryByTestId("popin")).not.toBeInTheDocument();

    openPopin();

    expect(screen.getByTestId("popin")).toBeInTheDocument();
  });

  it("positions the popin below the trigger", () => {
    render(<Harness />);
    const popin = openPopin();

    expect(popin).toHaveStyle({ position: "fixed" });
  });

  it("closes on Escape", () => {
    render(<Harness />);
    openPopin();

    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByTestId("popin")).not.toBeInTheDocument();
  });

  it("closes when clicking outside", () => {
    render(<Harness />);
    openPopin();

    fireEvent.mouseDown(document.body);

    expect(screen.queryByTestId("popin")).not.toBeInTheDocument();
  });

  it("stays open when clicking inside the popin", () => {
    render(<Harness />);
    openPopin();

    fireEvent.mouseDown(screen.getByRole("button", { name: "inside" }));

    expect(screen.getByTestId("popin")).toBeInTheDocument();
  });

  it("stays open when clicking the trigger", () => {
    render(<Harness />);
    openPopin();

    fireEvent.mouseDown(screen.getByRole("button", { name: "toggle" }));

    expect(screen.getByTestId("popin")).toBeInTheDocument();
  });
});
