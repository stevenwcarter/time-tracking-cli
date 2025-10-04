export const BorderedTableCell = ({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) => {
  return <td className={`border border-white px-2 py-0.5 ${className}`}>{children}</td>;
};
