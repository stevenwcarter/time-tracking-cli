import { Link } from 'react-router-dom';

export interface NavItemProps {
  label: string;
  href: string;
}
export const NavItem = (props: NavItemProps) => {
  const { label, href } = props;

  return (
    <Link
      to={href}
      className="py-2 px-4 text-white rounded bg-gray-800 hover:bg-gray-700 hover:underline"
    >
      {label}
    </Link>
  );
};

export default NavItem;
