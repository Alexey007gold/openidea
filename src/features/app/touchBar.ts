import { executeActionFrom, getAction, isActionEnabled, useActions } from "@/platform/actions";
import { useContextKeys, type ContextKeys } from "@/platform/context";
import { commands, isTauri, type TouchBarButton, type TouchBarIcon } from "@/platform/ipc";
import { hostOs } from "./host";

export const TOUCH_BAR_EVENT = "openidea://touch-bar";

type Slot = readonly [action: string, icon: TouchBarIcon];

interface Layout {
  slots: readonly Slot[];
  showTitle: boolean;
}

const DIFF: Layout = {
  slots: [
    ["diff.prevDifference", "previousDifference"],
    ["diff.nextDifference", "nextDifference"],
    ["diff.prevFile", "previousFile"],
    ["diff.nextFile", "nextFile"],
  ],
  showTitle: false,
};

const DEBUG: Layout = {
  slots: [
    ["debug.stepOver", "stepOver"],
    ["debug.stepInto", "stepInto"],
    ["debug.stepOut", "stepOut"],
    ["debug.resume", "resume"],
    ["debug.stop", "stop"],
  ],
  showTitle: true,
};

const NONE: Layout = { slots: [], showTitle: false };

function layout(ctx: ContextKeys): Layout {
  if (ctx.diffEditorFocus) return DIFF;
  if (ctx.debugSessionActive) return DEBUG;
  return NONE;
}

export function touchBarButtons(ctx: ContextKeys): TouchBarButton[] {
  const { slots, showTitle } = layout(ctx);
  return slots.flatMap(([id, icon]) => {
    const action = getAction(id);
    if (!action) return [];
    return [{ action: id, title: action.title, icon, showTitle, enabled: isActionEnabled(action, ctx) }];
  });
}

export function installTouchBar(): () => void {
  if (!isTauri()) return () => {};
  let disposed = false;
  let pushed = "";
  let timer: ReturnType<typeof setTimeout> | undefined;
  const cleanups: (() => void)[] = [];

  const push = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (disposed) return;
      const buttons = touchBarButtons(useContextKeys.getState());
      const key = JSON.stringify(buttons);
      if (key === pushed) return;
      pushed = key;
      void commands.touchBarSet(buttons);
    }, 50);
  };

  void hostOs().then(async (os) => {
    if (os !== "macos" || disposed) return;
    const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const off = await getCurrentWebviewWindow().listen<string>(TOUCH_BAR_EVENT, ({ payload }) => {
      void executeActionFrom("toolbar", payload);
    });
    if (disposed) {
      off();
      return;
    }
    cleanups.push(off, useContextKeys.subscribe(push), useActions.subscribe(push));
    push();
  });

  return () => {
    disposed = true;
    clearTimeout(timer);
    cleanups.forEach((cleanup) => cleanup());
    if (pushed && pushed !== "[]") void commands.touchBarSet([]);
  };
}
