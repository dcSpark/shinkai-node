export const GET_PUBLIC_KEY = 'GET_PUBLIC_KEY';
export const USE_REGISTRATION_CODE = 'USE_REGISTRATION_CODE';
export const PING_ALL = 'PING_ALL';

export interface Action {
    type: string;
    payload?: any;
  }