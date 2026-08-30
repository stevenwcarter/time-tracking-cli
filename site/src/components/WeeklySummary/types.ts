export interface Project {
  name: string;
  totalHours: number;
  notes: string[];
}

export interface Day {
  date: string;
  totalHours: number;
  deadTimeHours: number;
  projects: Project[];
  warnings: string[];
  startTime: string | null;
  endTime: string | null;
}

export interface WeekData {
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

/** One table row: a project's per-date hours plus its week total. */
export interface TableProject {
  name: string;
  dates: { [date: string]: number };
  total: number;
}

/** One table column: a date plus its short weekday label. */
export interface TableDate {
  date: string;
  dayOfWeek: string;
}
