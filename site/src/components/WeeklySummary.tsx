import { useNavigate } from 'react-router';
import DailyTotalsRow from './WeeklySummary/DailyTotalsRow';
import ProjectRow from './WeeklySummary/ProjectRow';
import type { WeekData } from './WeeklySummary/types';
import { useNotesLookup } from './WeeklySummary/useNotesLookup';
import { useWeeklyTableData } from './WeeklySummary/useWeeklyTableData';

interface WeeklySummaryProps {
  data: { weekDataForDate?: WeekData } | null;
  error?: unknown;
}

const WeeklySummary = ({ data, error }: WeeklySummaryProps) => {
  const weekData = data?.weekDataForDate;
  const navigate = useNavigate();

  const tableData = useWeeklyTableData(weekData);
  const { daysByDate, getNotesForProjectDate, formatNotesTooltip, copyDayNotes } =
    useNotesLookup(weekData);

  if (!weekData) {
    return (
      <div className="p-4 bg-gray-900 text-white rounded">
        {/* A failed query used to render this same "No data available",
            indistinguishable from a genuinely empty week. */}
        <p>{error ? 'Could not load this week. Please try again.' : 'No data available'}</p>
      </div>
    );
  }

  const editDate = (date: string) => {
    navigate(`/editor/${date}`);
  };

  return (
    <div className="p-6 bg-gray-900 text-white rounded shadow-lg">
      {/* Week Summary Header */}
      <div className="mb-6">
        <h2 className="text-2xl font-bold mb-2">Weekly Summary</h2>
        <div className="grid grid-cols-3 gap-4 text-center bg-gray-800 p-4 rounded">
          <div>
            <p className="text-gray-400 text-sm">Start Date</p>
            <p className="text-lg font-semibold">{weekData.startDate}</p>
          </div>
          <div>
            <p className="text-gray-400 text-sm">End Date</p>
            <p className="text-lg font-semibold">{weekData.endDate}</p>
          </div>
          <div>
            <p className="text-gray-400 text-sm">Total Hours</p>
            <p className="text-lg font-semibold">{weekData.totalHours.toFixed(2)}</p>
          </div>
        </div>
      </div>

      {/* Project Hours Table */}
      <div className="overflow-x-auto">
        <table className="w-full border-collapse">
          <thead>
            <tr className="bg-gray-800">
              <th className="border border-gray-600 p-3 text-left font-semibold">Project</th>
              {tableData.dates.map(({ date, dayOfWeek }) => (
                <th
                  key={date}
                  className="border cursor-pointer border-gray-600 p-3 text-center font-semibold min-w-[100px]"
                  onClick={() => editDate(date)}
                >
                  <div className="text-sm">{dayOfWeek}</div>
                  <div className="text-xs text-gray-400">{date}</div>
                </th>
              ))}
              <th className="border border-gray-600 p-3 text-center font-semibold bg-gray-700">
                Total
              </th>
            </tr>
          </thead>
          <tbody>
            {tableData.projects.map((project) => (
              <ProjectRow
                key={project.name}
                project={project}
                dates={tableData.dates}
                getNotesForProjectDate={getNotesForProjectDate}
                formatNotesTooltip={formatNotesTooltip}
                copyDayNotes={copyDayNotes}
              />
            ))}
            <DailyTotalsRow
              dates={tableData.dates}
              daysByDate={daysByDate}
              weekTotalHours={weekData.totalHours}
            />
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default WeeklySummary;
