import { useMemo } from 'react';
import { copyNotesToClipboard } from 'utils/clipboard';
import type { Day, WeekData } from './types';

interface NotesLookup {
  daysByDate: Map<string, Day>;
  getNotesForProjectDate: (projectName: string, date: string) => string[];
  formatNotesTooltip: (notes: string[]) => string;
  copyDayNotes: (projectName: string, date: string) => Promise<void>;
}

/**
 * Per-date/per-project note lookups for the table's hour cells, plus the
 * click handler that copies a cell's notes to the clipboard.
 */
export const useNotesLookup = (weekData: WeekData | undefined): NotesLookup => {
  // Pre-computed O(1) lookup map from date string to Day
  const daysByDate = useMemo(() => {
    const map = new Map<string, Day>();
    weekData?.days?.forEach((day) => map.set(day.date, day));
    return map;
  }, [weekData]);

  // Pre-computed O(1) lookup: notesByDate[date][projectName] = notes[]
  const notesByDate = useMemo(() => {
    const outer = new Map<string, Map<string, string[]>>();
    weekData?.days?.forEach((day) => {
      const inner = new Map<string, string[]>();
      day.projects?.forEach((p) => inner.set(p.name, p.notes));
      outer.set(day.date, inner);
    });
    return outer;
  }, [weekData]);

  const getNotesForProjectDate = (projectName: string, date: string): string[] => {
    return notesByDate.get(date)?.get(projectName) ?? [];
  };

  const formatNotesTooltip = (notes: string[]): string => {
    if (notes.length === 0) return 'No notes for this day';
    return notes.map((note) => `- ${note}`).join('\n');
  };

  const copyDayNotes = async (projectName: string, date: string) => {
    const notes = getNotesForProjectDate(projectName, date);
    if (notes.length === 0) return;
    await copyNotesToClipboard(notes, 'Notes copied to clipboard!');
  };

  return { daysByDate, getNotesForProjectDate, formatNotesTooltip, copyDayNotes };
};
