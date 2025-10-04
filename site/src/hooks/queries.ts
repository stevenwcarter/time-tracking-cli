import { gql } from '@apollo/client';

export const GET_DATA_FOR_DATE = gql`
  query DataForDate($date: String!) {
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
