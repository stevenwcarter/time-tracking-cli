import { copyNotesToClipboard } from 'utils/clipboard';

interface ParsedProject {
  name: string;
  totalHours: number;
  notes: string[];
}

interface ParsedDayData {
  date: string;
  totalHours: number;
  deadTimeHours: number;
  startTime: string | null;
  endTime: string | null;
  warnings: string[];
  projects: ParsedProject[];
}

export const DateSummary = (props: { parsedData: ParsedDayData }) => {
  const { parsedData } = props;

  const copyProjectNotesToClipboard = async (projectName: string, notes: string[]) => {
    if (notes.length === 0) return;
    await copyNotesToClipboard(notes, `${projectName} notes copied to clipboard!`);
  };

  return (
    <div className="px-2 overflow-y-auto">
      <h2 className="text-2xl font-bold mb-4">Summary for {parsedData.date}</h2>
      <p className="mb-2">Total Hours: {parsedData.totalHours?.toFixed(2)}</p>
      <p className="mb-2">Dead Time Hours: {parsedData.deadTimeHours?.toFixed(2)}</p>
      <p className="mb-4">
        Start Time: {parsedData.startTime} - End Time: {parsedData.endTime}
      </p>
      {parsedData.warnings.length > 0 && (
        <div className="mt-4 p-2 text-black bg-yellow-200 border border-yellow-400 rounded">
          <h3 className="font-semibold">Warnings:</h3>
          <ul className="list-disc list-inside">
            {parsedData.warnings.map((warning: string, index: number) => (
              <li key={index}>{warning}</li>
            ))}
          </ul>
        </div>
      )}
      <h3 className="text-xl font-semibold mt-8 mb-2">Projects:</h3>
      {parsedData.projects.map((project) => (
        <div key={project.name} className="mb-4">
          <div className="font-semibold flex text-lg">
            {project.name}
            <div className="text-sm ml-2 self-center">
              ({project.totalHours.toFixed(2)} {project.totalHours === 1 ? 'hour' : 'hours'})
            </div>
          </div>
          <ul
            className="list-disc list-inside cursor-pointer hover:bg-gray-800 p-2 rounded transition-colors"
            onClick={() => copyProjectNotesToClipboard(project.name, project.notes)}
            title="Click to copy notes to clipboard"
          >
            {project.notes.map((note: string, index: number) => (
              <li key={index}>{note}</li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
};

export default DateSummary;
