import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { registerActions } from "@/platform/actions";
import type { ContextKeys } from "@/platform/context";
import { touchBarButtons } from "../touchBar";

const noop = () => {};
let dispose: () => void;

beforeEach(() => {
  dispose = registerActions([
    { id: "diff.prevDifference", title: "Previous Difference", when: "diffEditorFocus", run: noop },
    { id: "diff.nextDifference", title: "Next Difference", when: "diffEditorFocus", run: noop },
    { id: "diff.prevFile", title: "Compare Previous File", when: "diffEditorFocus", run: noop },
    { id: "diff.nextFile", title: "Compare Next File", when: "diffEditorFocus", run: noop },
    { id: "debug.stepOver", title: "Step Over", when: "debugSessionPaused", run: noop },
    { id: "debug.stepInto", title: "Step Into", when: "debugSessionPaused", run: noop },
    { id: "debug.stepOut", title: "Step Out", when: "debugSessionPaused", run: noop },
    { id: "debug.resume", title: "Resume Program", when: "debugSessionPaused", run: noop },
    { id: "debug.stop", title: "Stop", when: "debugSessionActive", run: noop },
  ]);
});

afterEach(() => dispose());

const actions = (ctx: ContextKeys) => touchBarButtons(ctx).map((b) => b.action);

describe("touchBarButtons", () => {
  it("shows nothing outside a diff and a debug session", () => {
    expect(touchBarButtons({})).toEqual([]);
  });

  it("shows icon-only change and file navigation in a focused diff", () => {
    const buttons = touchBarButtons({ diffEditorFocus: true });
    expect(buttons.map((b) => [b.action, b.icon, b.showTitle, b.enabled])).toEqual([
      ["diff.prevDifference", "previousDifference", false, true],
      ["diff.nextDifference", "nextDifference", false, true],
      ["diff.prevFile", "previousFile", false, true],
      ["diff.nextFile", "nextFile", false, true],
    ]);
  });

  it("leads with Step Over while paused in the debugger", () => {
    const buttons = touchBarButtons({ debugSessionActive: true, debugSessionPaused: true });
    expect(buttons.map((b) => [b.action, b.title, b.showTitle, b.enabled])).toEqual([
      ["debug.stepOver", "Step Over", true, true],
      ["debug.stepInto", "Step Into", true, true],
      ["debug.stepOut", "Step Out", true, true],
      ["debug.resume", "Resume Program", true, true],
      ["debug.stop", "Stop", true, true],
    ]);
  });

  it("keeps the debug buttons but disables stepping while the program runs", () => {
    const buttons = touchBarButtons({ debugSessionActive: true, debugSessionPaused: false });
    expect(buttons.filter((b) => b.enabled).map((b) => b.action)).toEqual(["debug.stop"]);
    expect(buttons).toHaveLength(5);
  });

  it("prefers the diff while it has focus during a debug session", () => {
    expect(actions({ diffEditorFocus: true, debugSessionActive: true, debugSessionPaused: true })[0]).toBe(
      "diff.prevDifference",
    );
  });

  it("skips actions that are not registered", () => {
    dispose();
    expect(touchBarButtons({ diffEditorFocus: true })).toEqual([]);
    dispose = () => {};
  });
});
