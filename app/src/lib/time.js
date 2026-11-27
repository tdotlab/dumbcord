const clock = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const weekday = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const date = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" });
const dateYear = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" });
const full = new Intl.DateTimeFormat(undefined, { dateStyle: "full", timeStyle: "short" });

export const timeOf = (ms) => clock.format(ms);
export const fullTime = (ms) => full.format(ms);

function startOfDay(ms) {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function sameDay(a, b) {
  return startOfDay(a) === startOfDay(b);
}

/** "Today", "Yesterday", "Tuesday", "Mon, Nov 3", "Nov 3, 2025". */
export function dayLabel(ms, now = Date.now()) {
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return weekday.format(ms);
  return new Date(ms).getFullYear() === new Date(now).getFullYear()
    ? date.format(ms)
    : dateYear.format(ms);
}
