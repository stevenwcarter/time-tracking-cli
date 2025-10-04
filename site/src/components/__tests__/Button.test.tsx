import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Button from '../Button';
import { ButtonTypes } from '../Button/ButtonTypes';
import '@testing-library/jest-dom';

describe('SearchBuilder', () => {
  let user;

  beforeEach(() => {
    user = userEvent.setup();
  });

  it('renders without errors', async () => {
    render(<Button>test</Button>);
    const button = screen.getByRole('button');

    expect(button).toBeInTheDocument();
    expect(button).not.toHaveClass('m-0');
    expect(button).toHaveClass('m-2');
    expect(button).not.toHaveClass('px-2');
    expect(button).toHaveClass('px-6');
    // TODO - add spy on result
    await user.click(button);
  });

  it('renders primary variant by default', async () => {
    render(
      <div>
        <Button>test</Button>
        <Button type={ButtonTypes.PRIMARY}>test</Button>
      </div>,
    );
    const buttons = screen.getAllByRole('button');

    expect(buttons[0]).toBeInTheDocument();
    expect(buttons[1]).toBeInTheDocument();
    expect(buttons[0].classList).toStrictEqual(buttons[1].classList);
  });

  it('renders nomargin variant properly', async () => {
    render(<Button nomargin>test</Button>);
    const button = screen.getByRole('button');

    expect(button).toBeInTheDocument();
    expect(button).toHaveClass('m-0');
    expect(button).not.toHaveClass('m-2');
  });
  it('renders sm variant properly', async () => {
    render(<Button size={'sm'}>test</Button>);
    const button = screen.getByRole('button');

    expect(button).toBeInTheDocument();
    expect(button).not.toHaveClass('px-4');
    expect(button).toHaveClass('px-6');
  });
});
