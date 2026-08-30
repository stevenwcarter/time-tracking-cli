import { render } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seen: Date[] = [];
vi.mock('hooks/useWeekData', () => ({
  useWeekData: (d: Date) => {
    seen.push(d);
    return [undefined];
  },
}));

import Homepage from '../WeeklySummaryPage';

describe('WeeklySummaryPage default week', () => {
  beforeEach(() => {
    seen.length = 0;
  });

  it('parses an explicit :date param as the local calendar day, not UTC', () => {
    // The brief's sketch rendered <Homepage /> with no route, so
    // useParams() never produced an inputDate and only the harmless
    // `new Date()` branch ran. The actual bug is in the `inputDate ?
    // new Date(inputDate) : ...` branch, which needs a matching route so
    // useParams() actually yields a :date segment to parse.
    render(
      <MemoryRouter initialEntries={['/weekly-summary/2026-08-29']}>
        <Routes>
          <Route path="/weekly-summary/:date" element={<Homepage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(seen[0].getDate()).toBe(29);
    expect(seen[0].getMonth()).toBe(7);
  });
});
