/**
 * Shared "copy notes to the clipboard, then toast the outcome" sequence used
 * by both the day and weekly summary views. Callers own the decision of
 * whether there is anything worth copying and any placeholder text for an
 * empty selection (e.g. a tooltip fallback) — this module only formats the
 * notes it is given, writes them, and reports success or failure.
 */

import { toast } from 'react-toastify';

/** Join `notes` as a dash-prefixed list, copy it to the clipboard, and raise
 * a success toast with `successMessage`, or an error toast on failure. */
export const copyNotesToClipboard = async (
  notes: string[],
  successMessage: string,
): Promise<void> => {
  const formattedNotes = notes.map((note) => `- ${note}`).join('\n');

  try {
    await navigator.clipboard.writeText(formattedNotes);
    toast.success(successMessage, {
      position: 'top-right',
      autoClose: 2000,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
    });
  } catch {
    toast.error('Failed to copy to clipboard', {
      position: 'top-right',
      autoClose: 2000,
    });
  }
};
