import { ChangeEvent, useEffect, useState } from 'react';
import useDebounce from 'hooks/useDebounce';

interface AutoUpdateInputProps {
  serverValue: string;
  placeholder?: string;
  className?: string;
  type: 'text' | 'textarea';
  onChange: (value: string) => void;
}

export const AutoUpdateInput = (props: AutoUpdateInputProps) => {
  const { serverValue, onChange, type, placeholder, className } = props;
  const [currentValue, setCurrentValue] = useState(serverValue);

  // Effect to synchronize currentValue with serverValue if serverValue prop changes
  useEffect(() => {
    setCurrentValue(serverValue);
  }, [serverValue]);

  const debouncedValue = useDebounce(currentValue, 500);

  useEffect(() => {
    if (debouncedValue !== serverValue) {
      onChange(debouncedValue);
    }
  }, [debouncedValue, currentValue, serverValue, onChange]);

  const handleInputChange = (event: ChangeEvent<HTMLTextAreaElement | HTMLInputElement>) => {
    setCurrentValue(event.target.value);
  };

  if (type === 'textarea') {
    return (
      <textarea
        className={`p-2 m-1 w-full bg-slate-500/20 ${className}`}
        value={currentValue}
        onChange={handleInputChange}
        placeholder={placeholder}
      />
    );
  } else if (type === 'text') {
    return (
      <input
        type="text"
        className={`p-2 w-full bg-slate-500/20 ${className}`}
        value={currentValue}
        onChange={handleInputChange}
        placeholder={placeholder}
      />
    );
  } else {
    return 'Unimplemented input type';
  }
};

export default AutoUpdateInput;
