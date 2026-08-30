import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

vi.mock('hooks/useDateData', () => ({
  useDateData: () => ({ content: '', parsedData: null, updater: vi.fn() }),
}));

import DateEditorPage from '../DateEditorPage';

describe('DateEditorPage default date', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('defaults to the local calendar day, not the UTC one', () => {
    vi.setSystemTime(new Date(2026, 7, 29, 23, 30, 0));
    render(
      <MemoryRouter>
        <DateEditorPage />
      </MemoryRouter>,
    );
    expect(screen.getByRole('link', { name: /weekly summary/i })).toHaveAttribute(
      'href',
      '/weekly-summary/2026-08-29',
    );
  });

  it('parses an explicit :date param as the local calendar day, not UTC', () => {
    // A bare <MemoryRouter> with no <Route path="/editor/:date"> never
    // produces a :date param, so useParams() always returns {} and only
    // the todayDateString() fallback in DateEditorPage runs — the same
    // false-green shape caught and fixed in WeeklySummaryPage.test.tsx.
    // Route it for real so the parseDateString(date) branch is exercised.
    render(
      <MemoryRouter initialEntries={['/editor/2026-08-29']}>
        <Routes>
          <Route path="/editor/:date" element={<DateEditorPage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(screen.getByRole('link', { name: /weekly summary/i })).toHaveAttribute(
      'href',
      '/weekly-summary/2026-08-29',
    );
  });
});
