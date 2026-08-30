import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DateSelector from '../DateSelector';
import '@testing-library/jest-dom';

describe('DateSelector in a negative UTC offset', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('links to the local calendar day late in the evening', () => {
    // 23:30 local on 2026-08-29 is already 2026-08-30 in UTC.
    vi.setSystemTime(new Date(2026, 7, 29, 23, 30, 0));
    render(
      <MemoryRouter>
        <DateSelector date={new Date(2026, 7, 29, 23, 30, 0)} linkBase="/editor" />
      </MemoryRouter>,
    );
    const todayLink = screen.getByRole('link', { name: /today/i });
    expect(todayLink).toHaveAttribute('href', '/editor/2026-08-29');
  });
});
