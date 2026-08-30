import { useMemo } from 'react';
import { parseDateString } from 'utils/date';
import type { TableDate, TableProject, WeekData } from './types';

interface WeeklyTableData {
  projects: TableProject[];
  dates: TableDate[];
}

/**
 * Derive the table's rows and columns from `weekData`: one row per project
 * name (alphabetical) with per-date hours and a week total, and one column
 * per day (chronological) with its short weekday label.
 */
export const useWeeklyTableData = (weekData: WeekData | undefined): WeeklyTableData =>
  useMemo(() => {
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
      dates: sortedDates.map((day) => ({
        date: day.date,
        dayOfWeek: parseDateString(day.date).toLocaleDateString('en-US', { weekday: 'short' }),
      })),
    };
  }, [weekData]);
