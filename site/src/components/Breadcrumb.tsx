import { Approach, Client, Estimate } from 'types';
import Button from './Button';
import { Link } from 'react-router-dom';

interface BreadcrumbProps {
  approach?: Approach;
  client?: Client;
  estimate?: Estimate;
}

export const Breadcrumb = (props: BreadcrumbProps) => {
  const { approach, client, estimate } = props;

  return (
    <div className="flex items-center mb-2">
      <Link to={`/`}>
        <Button size={'sm'}>Home</Button>
      </Link>
      {client && client.uuid && (
        <>
          <div>&gt;</div>
          <Link to={`/client/${client.uuid}`}>
            <Button size={'sm'}>Client - {client.name}</Button>
          </Link>
        </>
      )}
      {estimate && estimate.uuid && (
        <>
          <div>&gt;</div>
          <Link to={`/estimate/${estimate.uuid}`}>
            <Button size={'sm'}>Estimate - {estimate.name}</Button>
          </Link>
        </>
      )}
      {approach && approach.uuid && (
        <>
          <div>&gt;</div>
          <Link to={`/approach/${approach.uuid}`}>
            <Button size={'sm'}>Approach - {approach.name}</Button>
          </Link>
        </>
      )}
    </div>
  );
};

export default Breadcrumb;
