export type ToastType = "info" | "success" | "error";

export interface ToastItem {
  id: number;
  msg: string;
  type: ToastType;
}

export const toasts = $state<ToastItem[]>([]);

let nextId = 1;

export function toast(msg: string, type: ToastType = "info", duration = 2800): void {
  const id = nextId++;
  toasts.push({ id, msg, type });
  window.setTimeout(() => dismiss(id), duration);
}

export function dismiss(id: number): void {
  const idx = toasts.findIndex((t) => t.id === id);
  if (idx >= 0) toasts.splice(idx, 1);
}
