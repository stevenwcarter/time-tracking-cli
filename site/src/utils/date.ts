/**
 * Date-string helpers for the `YYYY-MM-DD` boundary between `Date` objects
 * and the GraphQL API.
 *
 * Both directions work in **local** time on purpose. `toISOString()` formats
 * the UTC calendar day, so at 23:30 in any negative UTC offset it reports
 * tomorrow; `new Date('2026-08-29')` parses as UTC midnight, which in the
 * same zone is yesterday evening. A day file is named for the user's local
 * day, so both of those are off by one.
 */

/** Format `date` as `YYYY-MM-DD` using its local calendar day. */
export const toDateString = (date: Date): string => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
};

/** Parse a `YYYY-MM-DD` string as local midnight on that calendar day. */
export const parseDateString = (value: string): Date => new Date(`${value}T00:00:00`);

/** The current local calendar day as `YYYY-MM-DD`. */
export const todayDateString = (): string => toDateString(new Date());
