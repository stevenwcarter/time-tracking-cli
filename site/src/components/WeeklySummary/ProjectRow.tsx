import type { TableDate, TableProject } from './types';

interface ProjectRowProps {
  project: TableProject;
  dates: TableDate[];
  getNotesForProjectDate: (projectName: string, date: string) => string[];
  formatNotesTooltip: (notes: string[]) => string;
  copyDayNotes: (projectName: string, date: string) => void;
}

/** One project's row in the weekly table: an hours cell (with notes
 * tooltip, clickable to copy) per date, plus a week total cell. */
const ProjectRow = ({
  project,
  dates,
  getNotesForProjectDate,
  formatNotesTooltip,
  copyDayNotes,
}: ProjectRowProps) => (
  <tr className="hover:bg-gray-800">
    <td className="border border-gray-600 p-3 font-medium">{project.name}</td>
    {dates.map(({ date }) => {
      const notes = getNotesForProjectDate(project.name, date);
      const tooltipText = formatNotesTooltip(notes);
      const hasHours = project.dates[date] > 0;

      return (
        <td
          key={date}
          className={`border border-gray-600 p-3 text-center relative group ${hasHours ? 'cursor-pointer hover:bg-gray-700' : ''}`}
          title={tooltipText}
          onClick={hasHours ? () => copyDayNotes(project.name, date) : undefined}
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
);

export default ProjectRow;
