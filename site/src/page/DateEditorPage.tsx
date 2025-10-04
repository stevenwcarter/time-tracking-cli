import Button from 'components/Button';
import { DateEditor } from 'components/DateEditor';
import { useEffect, useMemo, useState } from 'react';
import { Link, useParams } from 'react-router';

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
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-800">
      <h1 className="text-4xl font-bold mb-4">{date}</h1>
      <div className="flex items-center mb-4">
        <input
          type="date"
          className="align-center border border-gray-300 bg-gray-200 text-gray-800 p-2 rounded-xl"
          value={newDate.toISOString().split('T')[0]}
          onChange={(e) => setNewDate(new Date(e.target.value))}
        />
        <Link to={`/editor/${newDate.toISOString().split('T')[0]}`}>
          <Button>Go to Date</Button>
        </Link>
      </div>
      <DateEditor key={currentDateString} date={dateObject} />
    </div>
  );
};

export default DateEditorPage;
