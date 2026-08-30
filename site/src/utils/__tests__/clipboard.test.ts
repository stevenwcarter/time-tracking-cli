import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'react-toastify';
import { copyNotesToClipboard } from '../clipboard';

vi.mock('react-toastify', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

describe('copyNotesToClipboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it('writes notes as a dash-prefixed list', async () => {
    await copyNotesToClipboard(['first', 'second'], 'Copied!');
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('- first\n- second');
  });

  it('raises a success toast with the given message', async () => {
    await copyNotesToClipboard(['a'], 'Project X notes copied!');
    expect(toast.success).toHaveBeenCalledWith('Project X notes copied!', expect.any(Object));
  });

  it('raises an error toast when the clipboard write rejects', async () => {
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) },
    });
    await copyNotesToClipboard(['a'], 'nope');
    expect(toast.error).toHaveBeenCalledWith('Failed to copy to clipboard', expect.any(Object));
    expect(toast.success).not.toHaveBeenCalled();
  });
});
