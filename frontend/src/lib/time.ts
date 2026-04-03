export function formatRelativeTime(isoDate: string): string {
  const date = new Date(isoDate);
  const diffMs = date.getTime() - Date.now();
  const absSeconds = Math.round(Math.abs(diffMs) / 1000);
  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

  if (absSeconds < 60) return rtf.format(Math.round(diffMs / 1000), 'second');
  if (absSeconds < 3600) return rtf.format(Math.round(diffMs / 60000), 'minute');
  if (absSeconds < 86400) return rtf.format(Math.round(diffMs / 3600000), 'hour');
  if (absSeconds < 2_592_000) return rtf.format(Math.round(diffMs / 86400000), 'day');
  if (absSeconds < 31_536_000) return rtf.format(Math.round(diffMs / 2_592_000), 'month');
  return rtf.format(Math.round(diffMs / 31_536_000), 'year');
}

export function formatDateTime(isoDate: string): string {
  const date = new Date(isoDate);
  return new Intl.DateTimeFormat('en', {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(date);
}
