import Button from 'components/Button';
import { DateEditor } from 'components/DateEditor';
import DateSelector from 'components/DateSelector';
import { useEffect, useMemo, useState } from 'react';
import { Link, useParams } from 'react-router';
import { parseDateString, todayDateString, toDateString } from 'utils/date';

export const DateEditorPage = () => {
  const { date } = useParams();
  const dateObject = useMemo(() => parseDateString(date || todayDateString()), [date]);
  const [newDate, setNewDate] = useState(dateObject);

  // Update newDate when the URL parameter changes
  useEffect(() => {
    setNewDate(dateObject);
  }, [dateObject]);

  const currentDateString = toDateString(dateObject);

  return (
    <div className="flex flex-col items-center min-h-screen bg-gray-800">
      <h1 className="text-4xl font-bold my-6">{date}</h1>
      <DateSelector date={newDate} linkBase="/editor" />
      <Link to={`/weekly-summary/${currentDateString}`}>
        <Button>Go to weekly summary</Button>
      </Link>
      <DateEditor key={currentDateString} date={dateObject} />
    </div>
  );
};

export default DateEditorPage;
