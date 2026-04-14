import { useMutation, useQuery } from '@apollo/client';
import { toast } from 'react-toastify';
import {
  FILE_CONTENT_FOR_DATE_QUERY,
  GET_DAY_DATA_FOR_DATE_QUERY,
  GET_WEEK_DATA_FOR_DATE_QUERY,
  UPDATE_FILE_CONTENT_FOR_DATE_MUTATION,
} from './queries';

export const useDateData = (date: Date) => {
  const dateString = date.toISOString().split('T')[0];
  const { data } = useQuery(FILE_CONTENT_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !date,
  });
  const { data: parsedData } = useQuery(GET_DAY_DATA_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !date,
  });
  const [updateDateData] = useMutation(UPDATE_FILE_CONTENT_FOR_DATE_MUTATION);

  const updater = (newContent: string) => {
    updateDateData({
      variables: { date: dateString, content: newContent },
      refetchQueries: [
        // Refetch daily queries for the current date
        { query: FILE_CONTENT_FOR_DATE_QUERY, variables: { date: dateString } },
        { query: GET_DAY_DATA_FOR_DATE_QUERY, variables: { date: dateString } },
        // Refetch weekly query for the week containing this date
        { query: GET_WEEK_DATA_FOR_DATE_QUERY, variables: { date: dateString } },
      ],
    }).catch((err) => {
      console.error('Failed to save time tracking data:', err);
      toast.error('Failed to save changes. Please try again.');
    });
  };

  return {
    content: data?.fileContentForDate || null,
    parsedData: parsedData?.dataForDate || null,
    updater,
  };
};
