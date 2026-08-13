import { api, ApiError, type DaemonStatus } from "../api/anetd";

export const appState = $state<{
  status: DaemonStatus | null;
  connected: boolean;
  checking: boolean;
  lastError: string;
}>({
  status: null,
  connected: false,
  checking: true,
  lastError: "",
});

let timer: number | undefined;
let started = false;

export async function refreshStatus(): Promise<boolean> {
  try {
    appState.status = await api.getStatus();
    appState.connected = true;
    appState.checking = false;
    appState.lastError = "";
    return true;
  } catch (e) {
    appState.connected = false;
    appState.checking = false;
    appState.lastError = e instanceof ApiError ? e.message : String(e);
    return false;
  }
}

function scheduleNext(intervalMs: number): void {
  window.clearTimeout(timer);
  timer = window.setTimeout(async () => {
    if (document.visibilityState !== "hidden") {
      await refreshStatus();
    }
    scheduleNext(intervalMs);
  }, intervalMs);
}

/** Start background status polling (idempotent, pauses when tab is hidden). */
export function startPolling(intervalMs = 3000): void {
  if (started) return;
  started = true;
  void refreshStatus();
  scheduleNext(intervalMs);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void refreshStatus();
  });
}
