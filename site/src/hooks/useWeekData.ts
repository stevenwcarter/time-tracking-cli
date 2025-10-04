import { useQuery } from '@apollo/client';
import { GET_WEEK_DATA_FOR_DATE_QUERY } from './queries';

export const useWeekData = (date: Date) => {
  const { data } = useQuery(GET_WEEK_DATA_FOR_DATE_QUERY, {
    variables: { date: date.toISOString().split('T')[0] },
    skip: !date,
  });

  return [data];
};
