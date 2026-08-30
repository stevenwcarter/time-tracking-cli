import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { parseDateString, todayDateString, toDateString } from '../date';

describe('toDateString', () => {
  it('formats from local components, not UTC', () => {
    // 2026-08-29 23:30 local. In any negative UTC offset this instant is
    // already 2026-08-30 in UTC, which is exactly the bug.
    const d = new Date(2026, 7, 29, 23, 30, 0);
    expect(toDateString(d)).toBe('2026-08-29');
  });

  it('zero-pads single-digit months and days', () => {
    expect(toDateString(new Date(2026, 0, 5))).toBe('2026-01-05');
  });

  it('round-trips with parseDateString', () => {
    expect(toDateString(parseDateString('2026-03-07'))).toBe('2026-03-07');
  });
});

describe('parseDateString', () => {
  it('parses at local midnight, not UTC midnight', () => {
    const d = parseDateString('2026-08-29');
    expect(d.getFullYear()).toBe(2026);
    expect(d.getMonth()).toBe(7);
    expect(d.getDate()).toBe(29);
    expect(d.getHours()).toBe(0);
  });
});

describe('todayDateString', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('uses the local calendar day at a late-evening instant', () => {
    vi.setSystemTime(new Date(2026, 7, 29, 23, 45, 0));
    expect(todayDateString()).toBe('2026-08-29');
  });
});
