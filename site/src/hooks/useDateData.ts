import { useMutation, useQuery } from '@apollo/client';
import { toast } from 'react-toastify';
import { toDateString } from 'utils/date';
import {
  FILE_CONTENT_FOR_DATE_QUERY,
  GET_DAY_DATA_FOR_DATE_QUERY,
  UPDATE_FILE_CONTENT_FOR_DATE_MUTATION,
} from './queries';

export const useDateData = (date: Date) => {
  const dateString = toDateString(date);
  const { data, error: contentError } = useQuery(FILE_CONTENT_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !dateString,
  });
  const { data: parsedData, error: parsedError } = useQuery(GET_DAY_DATA_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !dateString,
  });
  const [updateDateData] = useMutation(UPDATE_FILE_CONTENT_FOR_DATE_MUTATION);

  const updater = (newContent: string) => {
    updateDateData({
      variables: { date: dateString, content: newContent },
      // Operation names, not {query, variables} pairs. Pinning the variables
      // refetched only the cache entry for the edited date; a weekly-summary
      // page mounted with its week-start date is a different entry, and with
      // Apollo's default cache-first policy it kept serving pre-edit numbers
      // until a manual reload. Names refetch every currently-active instance
      // of each query, whatever date it was mounted with.
      // Pinned by src/hooks/__tests__/queries.test.ts.
      refetchQueries: ['FileContentForDate', 'DayDataForDate', 'WeekDataForDate'],
    }).catch((err) => {
      console.error('Failed to save time tracking data:', err);
      toast.error('Failed to save changes. Please try again.');
    });
  };

  // Widened to `Error` rather than left as `ApolloError` so a test can hand
  // the hook's consumers a plain `new Error(...)` without a cast.
  const error: Error | undefined = contentError ?? parsedError;

  return {
    // `||`, not `??`, deliberately left as it was. Switching would change how
    // an empty day file behaves, which is a separate question from error
    // surfacing and not this batch's to answer.
    content: data?.fileContentForDate || null,
    parsedData: parsedData?.dataForDate || null,
    updater,
    // Surfaced, not swallowed. The mutation path below has logged and toasted
    // its failures since it was written; the read path did neither, so a
    // failed load looked identical to an empty day and quietly disabled
    // saving for the rest of the session.
    error,
  };
};
