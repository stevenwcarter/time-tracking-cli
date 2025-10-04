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
      // Don't set lastSentData here - let it remain null so first user change will be detected
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
    // - Debounced data is different from what we last sent
    // - We're still on the same date that we started debouncing for
    console.log('Debounce effect triggered:', {
      isMounted: isMountedRef.current,
      hasInitialized,
      debouncedDataLength: debouncedData?.length,
      lastSentDataLength: lastSentData.current?.length,
      contentLength: content?.length || 0,
      dateMatch: currentDateRef.current.getTime() === date.getTime(),
      dataMatch: debouncedData === lastSentData.current,
      willUpdate:
        isMountedRef.current &&
        hasInitialized &&
        debouncedData !== lastSentData.current &&
        currentDateRef.current.getTime() === date.getTime(),
    });

    if (
      isMountedRef.current &&
      hasInitialized &&
      debouncedData !== lastSentData.current &&
      currentDateRef.current.getTime() === date.getTime()
    ) {
      console.log('Calling updater with length:', debouncedData?.length);
      lastSentData.current = debouncedData;
      updater(debouncedData);
    }
  }, [debouncedData, updater, date, content, hasInitialized]);

  return (
    <div className="w-full p-4 rounded shadow flex">
      <textarea
        value={localData}
        className="w-1/2 p-2 border rounded mr-4 bg-gray-900 text-white"
        onChange={(e) => setLocalData(e.target.value)}
        style={{ width: '50%', height: '100%' }}
      />
      <div className="w-1/2 p-4 border-l overflow-y-auto">
        <DateSummary parsedData={parsedData || { date: 'N/A', projects: [], warnings: [] }} />
      </div>
    </div>
  );
};
