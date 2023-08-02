import { GET_PUBLIC_KEY, USE_REGISTRATION_CODE, PING_ALL } from './types';

export const getPublicKey = (publicKey: string) => ({
  type: GET_PUBLIC_KEY,
  payload: publicKey
});

export const useRegistrationCode = () => ({
  type: USE_REGISTRATION_CODE
});

export const pingAll = (result: string) => ({
  type: PING_ALL,
  payload: result
});