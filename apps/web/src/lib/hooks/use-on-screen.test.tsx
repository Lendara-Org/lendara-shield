import { act, render, screen } from "@testing-library/react";
import { useRef } from "react";

import { useOnScreen } from "./use-on-screen";

type Callback = (entries: Partial<IntersectionObserverEntry>[]) => void;

function Probe() {
  const ref = useRef<HTMLDivElement>(null);
  const onScreen = useOnScreen(ref);
  return <div ref={ref} data-testid="probe" data-on-screen={onScreen} />;
}

describe("useOnScreen", () => {
  const original = globalThis.IntersectionObserver;
  afterEach(() => {
    globalThis.IntersectionObserver = original;
  });

  it("follows the element in and out of the viewport and disconnects on unmount", () => {
    let report: Callback = () => {};
    const disconnect = vi.fn();
    globalThis.IntersectionObserver = vi.fn(function (this: unknown, callback: Callback) {
      report = callback;
      return { observe: vi.fn(), disconnect };
    }) as unknown as typeof IntersectionObserver;

    const { unmount } = render(<Probe />);
    const probe = screen.getByTestId("probe");
    expect(probe).toHaveAttribute("data-on-screen", "true");
    act(() => report([{ isIntersecting: false }]));
    expect(probe).toHaveAttribute("data-on-screen", "false");
    act(() => report([{ isIntersecting: true }]));
    expect(probe).toHaveAttribute("data-on-screen", "true");
    unmount();
    expect(disconnect).toHaveBeenCalled();
  });

  it("counts as on screen without IntersectionObserver", () => {
    // @ts-expect-error: simulating a browser without the API
    delete globalThis.IntersectionObserver;
    render(<Probe />);
    expect(screen.getByTestId("probe")).toHaveAttribute("data-on-screen", "true");
  });
});
