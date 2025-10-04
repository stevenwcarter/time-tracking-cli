import { gql } from '@apollo/client';

export const GET_CONFIG_QUERY = gql`
  query {
    getConfig {
      apikey {
        enabled
      }
    }
  }
`;
