import { DateEditor } from 'components/DateEditor';
import DateSelector from 'components/DateSelector';
import { useEffect, useMemo, useState } from 'react';
import { useParams } from 'react-router';

export const DateEditorPage = () => {
  const { date } = useParams();
  const dateObject = useMemo(
    () => new Date(date || new Date().toISOString().split('T')[0]),
    [date],
  );
  const [newDate, setNewDate] = useState(dateObject);

  // Update newDate when the URL parameter changes
  useEffect(() => {
    setNewDate(dateObject);
  }, [dateObject]);

  const currentDateString = dateObject.toISOString().split('T')[0];

  return (
    <div className="flex flex-col items-center min-h-screen bg-gray-800">
      <h1 className="text-4xl font-bold my-6">{date}</h1>
      <DateSelector date={newDate} linkBase="/editor" />
      <DateEditor key={currentDateString} date={dateObject} />
    </div>
  );
};

export default DateEditorPage;
