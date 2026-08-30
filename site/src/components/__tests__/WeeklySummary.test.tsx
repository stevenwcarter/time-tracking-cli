import { fireEvent, render, screen, within } from '@testing-library/react';
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

// ---------------------------------------------------------------------------
// Characterization tests (T3): pin the current rendered output of
// WeeklySummary before it is split into hooks + row subcomponents. These
// must keep passing, unedited, through the extraction.
// ---------------------------------------------------------------------------

const weeklyFixture = {
  startDate: '2026-08-24',
  endDate: '2026-08-25',
  totalHours: 6.5,
  deadTimeHours: 0.5,
  days: [
    {
      // 2026-08-24 is a Monday. Parsed as UTC midnight (the bug this fixture
      // guards against) it would report as Sunday under the America/New_York
      // TZ pinned in vite.config.ts, since UTC-4 rolls midnight back to the
      // previous local evening.
      date: '2026-08-24',
      totalHours: 3.5,
      deadTimeHours: 0.5,
      warnings: [],
      startTime: null,
      endTime: null,
      projects: [
        { name: 'ProjectA', totalHours: 2.5, notes: ['Wrote the report', 'Fixed the bug'] },
        { name: 'ProjectB', totalHours: 1.0, notes: [] },
      ],
    },
    {
      date: '2026-08-25',
      totalHours: 3.0,
      deadTimeHours: 0,
      warnings: [],
      startTime: null,
      endTime: null,
      projects: [{ name: 'ProjectB', totalHours: 3.0, notes: [] }],
    },
  ],
  projectSummaries: [
    { name: 'ProjectA', totalHours: 2.5 },
    { name: 'ProjectB', totalHours: 4.0 },
  ],
};

describe('WeeklySummary rendering (characterization)', () => {
  it('renders the week summary header fields', () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weeklyFixture }} />
      </MemoryRouter>,
    );

    expect(screen.getByText('Start Date').nextElementSibling).toHaveTextContent('2026-08-24');
    expect(screen.getByText('End Date').nextElementSibling).toHaveTextContent('2026-08-25');
    expect(screen.getByText('Total Hours').nextElementSibling).toHaveTextContent('6.50');
  });

  it('renders weekday headers using local-time parsing, in date order', () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weeklyFixture }} />
      </MemoryRouter>,
    );

    const headerRow = screen.getAllByRole('row')[0];
    const headers = within(headerRow).getAllByRole('columnheader');

    // Project | Mon 2026-08-24 | Tue 2026-08-25 | Total
    expect(headers).toHaveLength(4);
    expect(headers[1]).toHaveTextContent('Mon');
    expect(headers[1]).toHaveTextContent('2026-08-24');
    expect(headers[2]).toHaveTextContent('Tue');
    expect(headers[2]).toHaveTextContent('2026-08-25');
  });

  it('renders project rows in alphabetical order with per-project totals', () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weeklyFixture }} />
      </MemoryRouter>,
    );

    const rows = screen.getAllByRole('row');
    expect(rows).toHaveLength(4); // header + ProjectA + ProjectB + Daily Totals

    const projectARow = rows[1];
    const projectBRow = rows[2];

    expect(within(projectARow).getByText('ProjectA')).toBeInTheDocument();
    expect(within(projectBRow).getByText('ProjectB')).toBeInTheDocument();

    const projectACells = within(projectARow).getAllByRole('cell');
    const projectBCells = within(projectBRow).getAllByRole('cell');

    // Project | day1 | day2 | Total
    expect(projectACells[1]).toHaveTextContent('2.50');
    expect(projectACells[2]).toHaveTextContent('-');
    expect(projectACells[3]).toHaveTextContent('2.50');

    expect(projectBCells[1]).toHaveTextContent('1.00');
    expect(projectBCells[2]).toHaveTextContent('3.00');
    expect(projectBCells[3]).toHaveTextContent('4.00');
  });

  it('renders the daily totals row', () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weeklyFixture }} />
      </MemoryRouter>,
    );

    const rows = screen.getAllByRole('row');
    const totalsRow = rows[3];

    expect(within(totalsRow).getByText('Daily Totals')).toBeInTheDocument();
    const totalsCells = within(totalsRow).getAllByRole('cell');
    expect(totalsCells[1]).toHaveTextContent('3.50');
    expect(totalsCells[2]).toHaveTextContent('3.00');
    expect(totalsCells[3]).toHaveTextContent('6.50');
  });

  it('sets the notes tooltip title per cell, with a no-notes fallback', () => {
    render(
      <MemoryRouter>
        <WeeklySummary data={{ weekDataForDate: weeklyFixture }} />
      </MemoryRouter>,
    );

    const rows = screen.getAllByRole('row');
    const projectARow = rows[1];
    const projectBRow = rows[2];

    const projectADay1Cell = within(projectARow).getAllByRole('cell')[1];
    expect(projectADay1Cell).toHaveAttribute('title', '- Wrote the report\n- Fixed the bug');

    const projectBDay1Cell = within(projectBRow).getAllByRole('cell')[1];
    expect(projectBDay1Cell).toHaveAttribute('title', 'No notes for this day');
  });
});
