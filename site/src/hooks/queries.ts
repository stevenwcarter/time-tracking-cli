import { gql } from '@apollo/client';

export const GET_DAY_DATA_FOR_DATE_QUERY = gql`
  query DayDataForDate($date: String!) {
    dataForDate(date: $date) {
      date
      totalHours
      deadTimeHours
      projects {
        name
        totalHours
        notes
      }
      warnings
      startTime
      endTime
    }
  }
`;

export const GET_WEEK_DATA_FOR_DATE_QUERY = gql`
  query WeekDataForDate($date: String!) {
    weekDataForDate(date: $date) {
      startDate
      endDate
      totalHours
      deadTimeHours
      days {
        date
        totalHours
        deadTimeHours
        projects {
          name
          totalHours
          notes
        }
        warnings
        startTime
        endTime
      }
      projectSummaries {
        name
        totalHours
      }
    }
  }
`;

export const FILE_CONTENT_FOR_DATE_QUERY = gql`
  query FileContentForDate($date: String!) {
    fileContentForDate(date: $date)
  }
`;

export const UPDATE_FILE_CONTENT_FOR_DATE_MUTATION = gql`
  mutation UpdateFileContentForDate($date: String!, $content: String!) {
    updateFileContent(date: $date, content: $content)
  }
`;
