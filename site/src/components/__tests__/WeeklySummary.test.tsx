import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { toast } from 'react-toastify';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import WeeklySummary from '../WeeklySummary';
import '@testing-library/jest-dom';

// WeeklySummary imports `useNavigate` from 'react-router' (not
// 'react-router-dom'); wrapping with `MemoryRouter` from the same package
// keeps them on one NavigationContext instance (see App.test.tsx for the
// dual-module split this avoids).

vi.mock('react-toastify', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const weekData = {
  startDate: '2026-08-24',
  endDate: '2026-08-30',
  totalHours: 1.5,
  deadTimeHours: 0,
  days: [
    {
      date: '2026-08-24',
      totalHours: 1.5,
      deadTimeHours: 0,
      warnings: [],
      startTime: null,
      endTime: null,
      projects: [{ name: 'ProjectA', totalHours: 1.5, notes: [] }],
    },
  ],
  projectSummaries: [{ name: 'ProjectA', totalHours: 1.5 }],
};

describe('WeeklySummary copy-notes cell', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it('does not copy or toast when clicking an hours cell with no notes', async () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weekData }} />
      </MemoryRouter>,
    );

    // Notes are empty, so the cell's tooltip title is the fallback text —
    // a unique locator for the clickable hours cell under test.
    const cell = screen.getByTitle('No notes for this day');
    fireEvent.click(cell);

    // Let the click handler's microtask (the awaited clipboard write) settle.
    await Promise.resolve();
    await Promise.resolve();

    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });
});
