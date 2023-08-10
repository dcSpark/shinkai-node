export const GET_PUBLIC_KEY = 'GET_PUBLIC_KEY';
export const USE_REGISTRATION_CODE = 'USE_REGISTRATION_CODE';
export const CREATE_REGISTRATION_CODE = 'CREATE_REGISTRATION_CODE';
export const REGISTRATION_ERROR = 'REGISTRATION_ERROR';
export const PING_ALL = 'PING_ALL';
export const CLEAR_REGISTRATION_CODE = 'CLEAR_REGISTRATION_CODE';

export interface Action {
    type: string;
    payload?: any;
  }