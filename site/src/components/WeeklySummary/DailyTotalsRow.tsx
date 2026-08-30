import type { Day, TableDate } from './types';

interface DailyTotalsRowProps {
  dates: TableDate[];
  daysByDate: Map<string, Day>;
  weekTotalHours: number;
}

/** The table's closing row: each day's total hours, plus the week total. */
const DailyTotalsRow = ({ dates, daysByDate, weekTotalHours }: DailyTotalsRowProps) => (
  <tr className="bg-gray-700 font-semibold">
    <td className="border border-gray-600 p-3">Daily Totals</td>
    {dates.map(({ date }) => {
      const dayData = daysByDate.get(date);
      return (
        <td key={date} className="border border-gray-600 p-3 text-center">
          {dayData ? dayData.totalHours.toFixed(2) : '-'}
        </td>
      );
    })}
    <td className="border border-gray-600 p-3 text-center bg-gray-600">
      {weekTotalHours.toFixed(2)}
    </td>
  </tr>
);

export default DailyTotalsRow;
