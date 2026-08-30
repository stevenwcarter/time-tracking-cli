import { useDateData } from 'hooks/useDateData';
import useDebounce from 'hooks/useDebounce';
import { useEffect, useState, useRef } from 'react';
import DateSummary from './DateSummary';

interface DateEditorProps {
  date: Date;
}

export const DateEditor = (props: DateEditorProps) => {
  const { date } = props;
  const { content, updater, parsedData } = useDateData(date);
  const [localData, setLocalData] = useState('');
  const [hasInitialized, setHasInitialized] = useState(false);
  const lastSentData = useRef<string | null>(null);
  const currentDateRef = useRef(date);
  const isMountedRef = useRef(true);

  // Initialize local data when content first loads
  useEffect(() => {
    if (content !== null && content !== undefined && !hasInitialized) {
      setLocalData(content);
      setHasInitialized(true);
      // Seed the baseline with what we just loaded. Leaving it null made the
      // debounce effect fire 500ms after every mount and re-write the file
      // with the bytes it had just read — a save the user never asked for,
      // and one that clobbered any external edit landing in that window.
      lastSentData.current = content;
    }
  }, [content, hasInitialized]);

  // Reset state when date changes
  useEffect(() => {
    if (currentDateRef.current.getTime() !== date.getTime()) {
      currentDateRef.current = date;
      lastSentData.current = null;
      setHasInitialized(false);
      setLocalData('');
    }
  }, [date]);

  // Update local data when server content changes (but only after initialization)
  useEffect(() => {
    if (hasInitialized && content !== null && content !== undefined) {
      setLocalData(content);
    }
  }, [content, hasInitialized]);

  // Cleanup on unmount
  useEffect(() => {
    isMountedRef.current = true; // Ensure it's set to true on mount
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const debouncedData = useDebounce(localData, 500);

  useEffect(() => {
    // Only send if:
    // - Component is still mounted and initialized
    // - The debounce has actually caught up to the latest local edit (see
    //   below) — otherwise this effect, which also re-runs the instant
    //   `hasInitialized` flips true, fires while `debouncedData` still holds
    //   its pre-load value and sends that stale/empty content instead
    // - Debounced data is different from what we last sent
    // - We're still on the same date that we started debouncing for
    if (
      isMountedRef.current &&
      hasInitialized &&
      // `hasInitialized` becoming true and `localData` being set to the
      // freshly-loaded content happen in the same render, but `debouncedData`
      // is separate state that only catches up 500ms later. Without this
      // check, that same-render effect run compares the loaded content
      // against a `debouncedData` still sitting at its pre-load value and
      // fires an immediate save of that stale value.
      localData === debouncedData &&
      debouncedData !== lastSentData.current &&
      currentDateRef.current.getTime() === date.getTime()
    ) {
      lastSentData.current = debouncedData;
      updater(debouncedData);
    }
  }, [debouncedData, localData, updater, date, hasInitialized]);

  return (
    <div className="w-full p-4 rounded shadow flex">
      <textarea
        value={localData}
        className="w-1/2 h-full p-2 border rounded mr-4 bg-gray-900 text-white"
        onChange={(e) => setLocalData(e.target.value)}
      />
      <div className="w-1/2 p-4 border-l overflow-y-auto">
        <DateSummary
          parsedData={
            parsedData || {
              date: 'N/A',
              totalHours: 0,
              deadTimeHours: 0,
              startTime: null,
              endTime: null,
              projects: [],
              warnings: [],
            }
          }
        />
      </div>
    </div>
  );
};
