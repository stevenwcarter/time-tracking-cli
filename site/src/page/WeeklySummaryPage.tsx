import DateSelector from 'components/DateSelector';
import WeeklySummary from 'components/WeeklySummary';
import { useWeekData } from 'hooks/useWeekData';
import { useParams } from 'react-router';

export const Homepage = () => {
  const { date: inputDate } = useParams();
  const date = inputDate ? new Date(inputDate) : new Date();
  const [data] = useWeekData(date);

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-800">
      <h1 className="text-4xl font-bold mb-4">Time Tracking Explorer</h1>
      <DateSelector date={date} linkBase="/weekly-summary" stepSize={7} />
      <WeeklySummary data={data} />
    </div>
  );
};

export default Homepage;
