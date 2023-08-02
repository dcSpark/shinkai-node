import axios from 'axios';
import { AppDispatch } from '../store/index';
import { getPublicKey, useRegistrationCode, pingAll } from '../store/actions';
import { ThunkAction } from 'redux-thunk';
import { Action } from 'redux';
import { RootState } from '../store';
import { AppThunk } from '../types';

const API_URL = 'http://localhost:3030/v1';

// export type AppThunk<ReturnType = void> = ThunkAction<ReturnType, RootState, unknown, Action<string>>;

export const fetchPublicKey = () => async (dispatch: AppDispatch) => {
  try {
    const response = await axios.get(`${API_URL}/get_public_key`);
    dispatch(getPublicKey(response.data));
  } catch (error) {
    console.error('Error fetching public key:', error);
  }
};

export const submitRegistrationCode = (code: string, profile_name: string, identity_pk: string, encryption_pk: string): AppThunk => async (dispatch: AppDispatch) => {
  try {
    await axios.post(`${API_URL}/use_registration_code`, {
      code,
      profile_name,
      identity_pk,
      encryption_pk
    });
    dispatch(useRegistrationCode());
  } catch (error) {
    console.error('Error using registration code:', error);
  }
};

export const pingAllNodes = () => async (dispatch: AppDispatch) => {
  try {
    const response = await axios.post(`${API_URL}/ping_all`);
    dispatch(pingAll(response.data.result));
  } catch (error) {
    console.error('Error pinging all nodes:', error);
  }
};
