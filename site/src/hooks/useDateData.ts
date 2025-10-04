import { useMutation, useQuery } from '@apollo/client';
import { FILE_CONTENT_FOR_DATE_QUERY, UPDATE_FILE_CONTENT_FOR_DATE_MUTATION } from './queries';

export const useDateData = (date: Date) => {
  const dateString = date.toISOString().split('T')[0];
  const { data } = useQuery(FILE_CONTENT_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !date,
  });
  const [updateDateData] = useMutation(UPDATE_FILE_CONTENT_FOR_DATE_MUTATION);

  const updater = (newContent: string) => {
    updateDateData({
      variables: { date: dateString, content: newContent },
      refetchQueries: [{ query: FILE_CONTENT_FOR_DATE_QUERY, variables: { date: dateString } }],
    }).catch(console.error);
  };

  return [data?.fileContentForDate || null, updater];
};
