import { render, screen } from '@testing-library/react';
import { ApolloClient, ApolloProvider, InMemoryCache } from '@apollo/client';
import { MemoryRouter } from 'react-router-dom';
import { vi } from 'vitest';
import WeeklySummaryPage from './page/WeeklySummaryPage';
import '@testing-library/jest-dom';

// DateSelector and WeeklySummary import `Link`/`useNavigate` from 'react-router'
// (not 'react-router-dom'), which triggers an ESM/CJS dual-module split in the
// Vitest environment and produces two separate NavigationContext instances.
// Mocking these child components avoids that split while still exercising the
// page-level rendering logic.
vi.mock('./components/DateSelector', () => ({ default: () => null }));
vi.mock('./components/WeeklySummary', () => ({ default: () => null }));

const testClient = new ApolloClient({
  cache: new InMemoryCache({ addTypename: false }),
  uri: '/graphql',
});

describe('App', () => {
  it('renders without errors', () => {
    render(
      <ApolloProvider client={testClient}>
        <MemoryRouter>
          <WeeklySummaryPage />
        </MemoryRouter>
      </ApolloProvider>,
    );

    expect(screen.getByText('Time Tracking Explorer')).toBeInTheDocument();
  });
});
