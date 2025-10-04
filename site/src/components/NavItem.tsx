import { Link } from 'react-router-dom';

export interface NavItemProps {
  label: string;
  href: string;
}
export const NavItem = (props: NavItemProps) => {
  const { label, href } = props;

  return (
    <li>
      <Link
        to={href}
        className="block py-2 px-4 text-white rounded hover:bg-gray-100 md:hover:bg-transparent md:border-0 md:hover:hover md:p-0 hover:underline"
      >
        {label}
      </Link>
    </li>
  );
};

export default NavItem;
