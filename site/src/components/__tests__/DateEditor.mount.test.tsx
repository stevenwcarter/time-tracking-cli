import { fireEvent, render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor mount', () => {
  it('does not save the content it just loaded', async () => {
    // Opening a day used to re-write its file ~500ms later with the exact
    // bytes just read, because the init effect deliberately left
    // lastSentData null. An external edit landing in that window was
    // clobbered by the stale in-browser copy.
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'loaded from server', parsedData: null, updater });

    render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);

    await new Promise((r) => setTimeout(r, 700));
    expect(updater).not.toHaveBeenCalled();
  });

  it('still saves once the user actually edits', async () => {
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'loaded from server', parsedData: null, updater });

    const { getByRole } = render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);
    await new Promise((r) => setTimeout(r, 700));

    const textarea = getByRole('textbox') as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: 'edited by the user' } });

    await waitFor(() => expect(updater).toHaveBeenCalledWith('edited by the user'), {
      timeout: 2000,
    });
  });
});
