import { useQuery } from '@apollo/client';
import { toDateString } from 'utils/date';
import { GET_WEEK_DATA_FOR_DATE_QUERY } from './queries';

export const useWeekData = (date: Date) => {
  const { data } = useQuery(GET_WEEK_DATA_FOR_DATE_QUERY, {
    variables: { date: toDateString(date) },
    skip: !date,
  });

  return [data];
};
