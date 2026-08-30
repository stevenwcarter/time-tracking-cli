import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor load failure', () => {
  it('tells the user and refuses input instead of silently dropping it', () => {
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({
      content: null,
      parsedData: null,
      updater: vi.fn(),
      error: new Error('network down'),
    });

    const { getByRole, getByText } = render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);

    expect(getByText(/could not load/i)).toBeInTheDocument();
    expect(getByRole('textbox')).toBeDisabled();
  });
});
