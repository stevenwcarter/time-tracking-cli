import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor debounced save', () => {
  it('does not re-save when only content changes identity', async () => {
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'a', parsedData: null, updater, error: undefined });

    const date = new Date('2026-08-29T00:00:00');
    const { rerender } = render(<DateEditor date={date} />);

    // Mount used to settle into an initial debounced save of the content it
    // had just loaded; that quirk is fixed, so the settle window must now
    // pass with no save at all.
    await new Promise((r) => setTimeout(r, 600));
    expect(updater).not.toHaveBeenCalled();

    // Same content value, new object identity from a refetch.
    spy.mockReturnValue({ content: 'a', parsedData: null, updater, error: undefined });
    rerender(<DateEditor date={date} />);

    await new Promise((r) => setTimeout(r, 600));
    expect(updater).not.toHaveBeenCalled();
  });
});
