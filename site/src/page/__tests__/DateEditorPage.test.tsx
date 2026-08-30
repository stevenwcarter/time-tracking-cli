import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
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
});
