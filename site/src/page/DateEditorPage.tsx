import { DateEditor } from 'components/DateEditor';
import { useParams } from 'react-router';

export const DateEditorPage = () => {
  const { date } = useParams();
  const dateObject = new Date(date || new Date().toISOString().split('T')[0]);

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-800">
      <h1 className="text-4xl font-bold mb-4">Welcome to the DateEditorPage</h1>
      <p className="text-lg text-gray-300">This is the date editor</p>
      <DateEditor date={dateObject} />
    </div>
  );
};

export default DateEditorPage;
