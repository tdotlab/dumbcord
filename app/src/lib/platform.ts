// The few places where the desktop/phone app and a plain browser differ.

export const isTauri = "__TAURI_INTERNALS__" in window;

export async function openExternal(url: string) {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

let permission: Promise<boolean> | null = null;

async function canNotify(): Promise<boolean> {
  if (isTauri) {
    const n = await import("@tauri-apps/plugin-notification");
    if (await n.isPermissionGranted()) return true;
    return (await n.requestPermission()) === "granted";
  }
  if (!("Notification" in window)) return false;
  if (Notification.permission === "granted") return true;
  if (Notification.permission === "denied") return false;
  return (await Notification.requestPermission()) === "granted";
}

/** Ask once, early, from a user gesture (browsers require that). */
export function primeNotifications() {
  permission ??= canNotify().catch(() => false);
}

export async function notify(title: string, body: string) {
  permission ??= canNotify().catch(() => false);
  if (!(await permission)) return;
  if (isTauri) {
    const n = await import("@tauri-apps/plugin-notification");
    n.sendNotification({ title, body });
  } else {
    new Notification(title, { body, silent: false });
  }
}

export function appIsVisible(): boolean {
  return document.visibilityState === "visible" && document.hasFocus();
}
