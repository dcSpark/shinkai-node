import { SetupDetailsState } from './reducers';
import { GET_PUBLIC_KEY, USE_REGISTRATION_CODE, PING_ALL, REGISTRATION_ERROR } from './types';

export const getPublicKey = (publicKey: string) => ({
  type: GET_PUBLIC_KEY,
  payload: publicKey
});

export const useRegistrationCode = (setupData: SetupDetailsState) => ({
  type: USE_REGISTRATION_CODE,
  payload: setupData
});

export const registrationError = (error: string) => ({
  type: REGISTRATION_ERROR,
  payload: error
});

export const pingAll = (result: string) => ({
  type: PING_ALL,
  payload: result
});