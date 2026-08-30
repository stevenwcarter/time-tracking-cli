import type { DocumentNode, OperationDefinitionNode } from 'graphql';
import { describe, expect, it } from 'vitest';
import {
  FILE_CONTENT_FOR_DATE_QUERY,
  GET_DAY_DATA_FOR_DATE_QUERY,
  GET_WEEK_DATA_FOR_DATE_QUERY,
} from '../queries';

const operationName = (doc: DocumentNode) =>
  (doc.definitions[0] as OperationDefinitionNode).name?.value;

describe('query operation names', () => {
  // useDateData's refetchQueries names these as strings. A rename here with
  // no matching rename there makes Apollo refetch nothing, silently.
  it('match the strings useDateData refetches by', () => {
    expect(operationName(FILE_CONTENT_FOR_DATE_QUERY)).toBe('FileContentForDate');
    expect(operationName(GET_DAY_DATA_FOR_DATE_QUERY)).toBe('DayDataForDate');
    expect(operationName(GET_WEEK_DATA_FOR_DATE_QUERY)).toBe('WeekDataForDate');
  });
});
