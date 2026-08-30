import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { mutate, useQueryMock } = vi.hoisted(() => ({
  mutate: vi.fn(),
  useQueryMock: vi.fn(),
}));

vi.mock('@apollo/client', () => ({
  gql: (strings: TemplateStringsArray, ...values: unknown[]) =>
    String.raw({ raw: strings }, ...values),
  useQuery: (...args: unknown[]) => useQueryMock(...args),
  useMutation: () => [mutate],
}));

import { useDateData } from '../useDateData';

describe('useDateData.updater', () => {
  beforeEach(() => {
    mutate.mockReset();
    mutate.mockResolvedValue({});
    useQueryMock.mockReset();
    useQueryMock.mockReturnValue({ data: undefined, error: undefined });
  });

  it('refetches by operation name, not pinned to the edited date', () => {
    // Pinning {date: '2026-08-27'} left the weekly-summary page's own cache
    // entry — mounted with the week-start date — untouched, so navigating
    // back served pre-edit numbers until a manual reload.
    const { result } = renderHook(() => useDateData(new Date('2026-08-27T00:00:00')));

    result.current.updater('new content');

    expect(mutate).toHaveBeenCalledTimes(1);
    expect(mutate.mock.calls[0][0]).toMatchObject({
      variables: { date: '2026-08-27', content: 'new content' },
      refetchQueries: ['FileContentForDate', 'DayDataForDate', 'WeekDataForDate'],
    });
  });
});
