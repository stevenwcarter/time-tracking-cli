// export const testdata = {
//   date: '2025-10-04',
//   totalHours: 3,
//   deadTimeHours: 0,
//   projects: [
//     {
//       name: 'admin',
//       totalHours: 1,
//       notes: ['this is a test'],
//     },
//     {
//       name: 'project2',
//       totalHours: 2,
//       notes: ['another test', 'yet another test task'],
//     },
//   ],
//   warnings: [],
//   startTime: '12:00',
//   endTime: '3:00',
// };

export const DateSummary = (props: { parsedData: any }) => {
  const { parsedData } = props;

  return (
    <div className="px-2 overflow-y-auto">
      <h2 className="text-2xl font-bold mb-4">Summary for {parsedData.date}</h2>
      <p className="mb-2">Total Hours: {parsedData.totalHours}</p>
      <p className="mb-2">Dead Time Hours: {parsedData.deadTimeHours}</p>
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
      {parsedData.projects.map((project: any) => (
        <div key={project.name} className="mb-4">
          <div className="font-semibold flex text-lg">
            {project.name}
            <div className="text-sm ml-2 self-center">
              ({project.totalHours} {project.totalHours === 1 ? 'hour' : 'hours'})
            </div>
          </div>
          <ul className="list-disc list-inside">
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
