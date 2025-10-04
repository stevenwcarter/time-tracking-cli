export enum ButtonTypes {
  INFO = 'INFO',
  DEFAULT = 'DEFAULT',
  DANGER = 'DANGER',
  WARN = 'WARN',
  PRIMARY = 'PRIMARY',
  SUCCESS = 'SUCCESS',
}

export const getVariant = (type?: ButtonTypes, disabled?: boolean) => {
  switch (type) {
    case 'SUCCESS':
      return disabled
        ? ''
        : 'border-green-600 hover:border-green-500 bg-green-700 hover:bg-green-600 text-white';
    case 'DANGER':
      return 'border-red-600 hover:border-red-500 bg-red-700 hover:bg-red-600 text-white';
    case 'INFO':
      return 'border-blue-600 hover:border-blue-500 bg-blue-700 hover:bg-blue-600 text-white';
    case 'WARN':
      return 'border-yellow-600 hover:border-yellow-500 bg-yellow-700 hover:bg-yellow-600 text-white';
    case 'PRIMARY':
    default:
      return 'border-slate-500 hover:border-slate-400 bg-slate-600 hover:bg-slate-500';
  }
};
