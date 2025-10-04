import { useDateData } from 'hooks/useDateData';
import useDebounce from 'hooks/useDebounce';
import { useEffect, useState, useRef } from 'react';

interface DateEditorProps {
  date: Date;
}

export const DateEditor = (props: DateEditorProps) => {
  const { date } = props;
  const [serverData, setServerData] = useDateData(date);
  const [localData, setLocalData] = useState(serverData);
  const lastSentData = useRef<string | null>(null);

  useEffect(() => {
    setLocalData(serverData);
  }, [serverData]);

  const debouncedData = useDebounce(localData, 500);

  useEffect(() => {
    // Only send if the debounced data is different from what we last sent
    // and different from the current server data
    if (
      debouncedData !== null &&
      debouncedData !== lastSentData.current &&
      debouncedData !== serverData
    ) {
      lastSentData.current = debouncedData;
      setServerData(debouncedData || '');
    }
  }, [debouncedData, serverData, setServerData]);

  return (
    <textarea
      value={localData || ''}
      onChange={(e) => setLocalData(e.target.value)}
      style={{ width: '100%', height: '300px' }}
    />
  );
};
