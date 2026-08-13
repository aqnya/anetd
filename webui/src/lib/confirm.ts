export interface ConfirmOptions {
  title: string;
  message?: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
}

interface ConfirmRequest {
  opts: ConfirmOptions;
  resolve: (value: boolean) => void;
}

export let confirmState = $state<ConfirmRequest | null>(null);

export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    confirmState = { opts, resolve };
  });
}

export function settleConfirm(value: boolean): void {
  if (confirmState) {
    confirmState.resolve(value);
    confirmState = null;
  }
}
