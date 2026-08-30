import { render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor debounced save', () => {
  it('does not re-save when only content changes identity', async () => {
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'a', parsedData: null, updater });

    const date = new Date('2026-08-29T00:00:00');
    const { rerender } = render(<DateEditor date={date} />);

    // Mount settles into an initial debounced save regardless of this fix
    // (a separate, pre-existing quirk unrelated to the content dep — see
    // task-8-9-report.md). Let it finish before exercising the case this
    // test actually targets.
    await waitFor(() => expect(updater).toHaveBeenCalledWith('a'));
    updater.mockClear();

    // Same content value, new object identity from a refetch.
    spy.mockReturnValue({ content: 'a', parsedData: null, updater });
    rerender(<DateEditor date={date} />);

    await new Promise((r) => setTimeout(r, 600));
    expect(updater).not.toHaveBeenCalled();
  });
});
