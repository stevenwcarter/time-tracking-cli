import React from 'react';
import clsx from 'clsx';
import { ButtonTypes } from './ButtonTypes';

export interface ButtonProps {
  'aria-label'?: string;
  type?: ButtonTypes;
  disabled?: boolean;
  nomargin?: boolean;
  children: any;
  size?: 'sm' | 'lg';
  onClick?: (e: React.MouseEvent<HTMLElement>) => void;
  block?: boolean;
  title?: string;
  className?: string;
  truetype?: 'button' | 'submit' | 'reset' | undefined;
}

export const MyButton = (props: ButtonProps) => {
  const { block, disabled, children, nomargin, type, className, truetype, ...remainingProps } =
    props;

  const classes = clsx(
    // 'text-black',
    'transition',
    'text-white',
    'cursor-pointer',
    'bg-gray-900',
    !disabled && 'cursor-pointer',
    disabled && 'bg-gray text-black',
    'duration-200',
    'ease-in-out',
    'focus:border focus:border-medium-pink',
    'whitespace-nowrap',
    !disabled && 'hover:shadow-lg hover:shadow-slate-500/40',
    nomargin ? 'm-0' : 'm-2',
    'rounded-l-full rounded-r-full',
    'py-2 px-6',
    props.size === 'sm' ? 'leading-[1.1875rem]' : 'text-sm',
    // getVariant(type, disabled),
    // block && 'w-full',
    className,
  );

  return (
    <button disabled={disabled} className={classes} type={truetype} {...remainingProps}>
      {props.size !== 'sm' && (
        <div className="h-[3rem] flex-shrink-0 inline-flex items-center">{children}</div>
      )}
      {props.size === 'sm' && children}
    </button>
  );
};

export default MyButton;
