import { useState, useEffect } from 'react';
import Button from './Button';
import { Link } from 'react-router';

interface DateSelectorProps {
  date?: Date;
  linkBase: string;
  stepSize?: number; // Number of days to step forward/backward
}

export const DateSelector = (props: DateSelectorProps) => {
  const { stepSize = 1 } = props;
  const [date, setDate] = useState(props.date || new Date());

  // Update internal date state when props.date changes (e.g., from URL navigation)
  useEffect(() => {
    if (props.date) {
      setDate(props.date);
    }
  }, [props.date]);

  // Helper function to format date for URL and input
  const formatDate = (d: Date) => d.toISOString().split('T')[0];

  // Helper function to add/subtract days
  const addDays = (d: Date, days: number) => {
    const result = new Date(d);
    result.setDate(result.getDate() + days);
    return result;
  };

  // Get previous and next dates based on step size
  const prevDate = addDays(date, -stepSize);
  const nextDate = addDays(date, stepSize);
  const today = new Date();

  return (
    <div className="flex items-center gap-2 mb-4">
      {/* Previous button */}
      <Link to={`${props.linkBase}/${formatDate(prevDate)}`}>
        <Button className="rounded-xl px-3 py-2">&#8249; Prev</Button>
      </Link>

      {/* Today button */}
      <Link to={`${props.linkBase}/${formatDate(today)}`}>
        <Button className="rounded-xl px-3 py-2 bg-blue-600 hover:bg-blue-700">Today</Button>
      </Link>

      {/* Date input */}
      <input
        type="date"
        className="align-center border border-gray-300 bg-gray-200 text-gray-800 p-4 rounded-xl"
        value={formatDate(date)}
        onChange={(e) => setDate(new Date(e.target.value))}
      />

      {/* Go to Date button */}
      <Link to={`${props.linkBase}/${formatDate(date)}`}>
        <Button className="rounded-xl">Go to Date</Button>
      </Link>

      {/* Next button */}
      <Link to={`${props.linkBase}/${formatDate(nextDate)}`}>
        <Button className="rounded-xl px-3 py-2">Next &#8250;</Button>
      </Link>
    </div>
  );
};

export default DateSelector;
