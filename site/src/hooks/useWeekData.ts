import { useQuery } from '@apollo/client';
import { toDateString } from 'utils/date';
import { GET_WEEK_DATA_FOR_DATE_QUERY } from './queries';

export const useWeekData = (date: Date) => {
  const { data, error } = useQuery(GET_WEEK_DATA_FOR_DATE_QUERY, {
    variables: { date: toDateString(date) },
    skip: !date,
  });

  // Positional so existing `const [data] = useWeekData(date)` call sites keep
  // working unchanged.
  return [data, error] as const;
};
