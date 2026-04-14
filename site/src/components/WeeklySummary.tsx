import { useMemo } from 'react';
import { toast } from 'react-toastify';
import { useNavigate } from 'react-router';

interface Project {
  name: string;
  totalHours: number;
  notes: string[];
}

interface Day {
  date: string;
  totalHours: number;
  deadTimeHours: number;
  projects: Project[];
  warnings: string[];
  startTime: string | null;
  endTime: string | null;
}

interface WeekData {
  startDate: string;
  endDate: string;
  totalHours: number;
  deadTimeHours: number;
  days: Day[];
  projectSummaries: Array<{
    name: string;
    totalHours: number;
  }>;
}

interface WeeklySummaryProps {
  data: { weekDataForDate?: WeekData } | null;
}

const WeeklySummary = ({ data }: WeeklySummaryProps) => {
  const weekData = data?.weekDataForDate;
  const navigate = useNavigate();

  const tableData = useMemo(() => {
    if (!weekData || !weekData.days || !Array.isArray(weekData.days)) {
      return { projects: [], dates: [] };
    }

    // Get all unique project names
    const allProjects = new Set<string>();
    weekData.days.forEach((day) => {
      if (day.projects && Array.isArray(day.projects)) {
        day.projects.forEach((project) => {
          allProjects.add(project.name);
        });
      }
    });

    // Sort dates
    const sortedDates = [...weekData.days].sort(
      (a, b) => new Date(a.date).getTime() - new Date(b.date).getTime(),
    );

    // Create project data with hours for each date
    const projects = Array.from(allProjects)
      .sort()
      .map((projectName) => {
        const projectData: { [date: string]: number } = {};
        let totalHours = 0;

        sortedDates.forEach((day) => {
          const project = day.projects?.find((p) => p.name === projectName);
          const hours = project ? project.totalHours : 0;
          projectData[day.date] = hours;
          totalHours += hours;
        });

        return {
          name: projectName,
          dates: projectData,
          total: totalHours,
        };
      });

    return {
      projects,
      dates: sortedDates.map((day) => {
        // Fix the day name calculation - create date correctly
        const date = new Date(day.date + 'T00:00:00');
        return {
          date: day.date,
          dayOfWeek: date.toLocaleDateString('en-US', { weekday: 'short' }),
        };
      }),
    };
  }, [weekData]);

  // Pre-computed O(1) lookup map from date string to Day
  const daysByDate = useMemo(() => {
    const map = new Map<string, Day>();
    weekData?.days?.forEach((day) => map.set(day.date, day));
    return map;
  }, [weekData]);

  // Pre-computed O(1) lookup: notesByDate[date][projectName] = notes[]
  const notesByDate = useMemo(() => {
    const outer = new Map<string, Map<string, string[]>>();
    weekData?.days?.forEach((day) => {
      const inner = new Map<string, string[]>();
      day.projects?.forEach((p) => inner.set(p.name, p.notes));
      outer.set(day.date, inner);
    });
    return outer;
  }, [weekData]);

  // Helper function to get notes for a specific project on a specific date
  const getNotesForProjectDate = (projectName: string, date: string): string[] => {
    return notesByDate.get(date)?.get(projectName) ?? [];
  };

  // Helper function to format notes as tooltip text
  const formatNotesTooltip = (notes: string[]): string => {
    if (notes.length === 0) return 'No notes for this day';
    return notes.map((note) => `- ${note}`).join('\n');
  };

  // Helper function to copy notes to clipboard
  const copyNotesToClipboard = async (projectName: string, date: string) => {
    const notes = getNotesForProjectDate(projectName, date);
    const formattedNotes = formatNotesTooltip(notes);

    try {
      await navigator.clipboard.writeText(formattedNotes);
      toast.success('Notes copied to clipboard!', {
        position: 'top-right',
        autoClose: 2000,
        hideProgressBar: false,
        closeOnClick: true,
        pauseOnHover: true,
        draggable: true,
      });
    } catch (err) {
      toast.error('Failed to copy to clipboard', {
        position: 'top-right',
        autoClose: 2000,
      });
    }
  };

  if (!weekData) {
    return (
      <div className="p-4 bg-gray-900 text-white rounded">
        <p>No data available</p>
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
              <tr key={project.name} className="hover:bg-gray-800">
                <td className="border border-gray-600 p-3 font-medium">{project.name}</td>
                {tableData.dates.map(({ date }) => {
                  const notes = getNotesForProjectDate(project.name, date);
                  const tooltipText = formatNotesTooltip(notes);
                  const hasHours = project.dates[date] > 0;

                  return (
                    <td
                      key={date}
                      className={`border border-gray-600 p-3 text-center relative group ${hasHours ? 'cursor-pointer hover:bg-gray-700' : ''}`}
                      title={tooltipText}
                      onClick={
                        hasHours ? () => copyNotesToClipboard(project.name, date) : undefined
                      }
                    >
                      {hasHours ? project.dates[date].toFixed(2) : '-'}
                      {/* Tooltip */}
                      {hasHours && (
                        <div className="absolute invisible group-hover:visible bg-gray-800 text-white text-xs rounded p-2 bottom-full left-1/2 transform -translate-x-1/2 mb-2 w-64 z-10 border border-gray-600 shadow-lg whitespace-pre-line">
                          {tooltipText}
                          <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-l-4 border-r-4 border-t-4 border-transparent border-t-gray-800"></div>
                        </div>
                      )}
                    </td>
                  );
                })}
                <td className="border border-gray-600 p-3 text-center font-semibold bg-gray-700">
                  {project.total.toFixed(2)}
                </td>
              </tr>
            ))}
            {/* Totals row */}
            <tr className="bg-gray-700 font-semibold">
              <td className="border border-gray-600 p-3">Daily Totals</td>
              {tableData.dates.map(({ date }) => {
                const dayData = daysByDate.get(date);
                return (
                  <td key={date} className="border border-gray-600 p-3 text-center">
                    {dayData ? dayData.totalHours.toFixed(2) : '-'}
                  </td>
                );
              })}
              <td className="border border-gray-600 p-3 text-center bg-gray-600">
                {weekData.totalHours.toFixed(2)}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default WeeklySummary;
