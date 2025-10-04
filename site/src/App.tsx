import LoadingSpinner from 'components/LoadingSpinner';
import React, { Suspense } from 'react';
import { ApolloClient, ApolloProvider, InMemoryCache } from '@apollo/client';
import { ToastContainer } from 'react-toastify';
import { RouterProvider, createBrowserRouter } from 'react-router-dom';
import 'react-toastify/dist/ReactToastify.css';

const PageTemplate = React.lazy(() => import('page/PageTemplate'));
const Homepage = React.lazy(() => import('page/Homepage'));
const DateEditorPage = React.lazy(() => import('page/DateEditorPage'));

const apolloClient = new ApolloClient({
  cache: new InMemoryCache({ addTypename: false }),
  uri: '/graphql',
});

const router = createBrowserRouter([
  {
    path: '/',
    element: <PageTemplate />,
    children: [
      {
        index: true,
        element: <Homepage />,
      },
      { path: 'editor', element: <DateEditorPage /> },
      { path: 'editor/:date', element: <DateEditorPage /> },
      // {
      //   path: 'client/:clientUuid',
      //   element: <Client />,
      // },
    ],
  },
]);

export const App = () => {
  return (
    <React.StrictMode>
      <ApolloProvider client={apolloClient}>
        <Suspense fallback={<LoadingSpinner />}>
          <ToastContainer />
          <RouterProvider router={router} />
        </Suspense>
      </ApolloProvider>
    </React.StrictMode>
  );
};

export default App;
