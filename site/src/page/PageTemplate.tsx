import NavItem from 'components/NavItem';
import { Outlet } from 'react-router-dom';

export const PageTemplate = () => (
  <div className="h-full flex flex-col text-white">
    <div className="flex flex-row bg-gray-900 p-4 space-x-4">
      <NavItem label="Home" href="/" />
      <NavItem label="Editor" href="/editor" />
    </div>
    <div className="flex flex-col p-4 md:p-10">
      <Outlet />
    </div>
  </div>
);

export default PageTemplate;
