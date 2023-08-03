import axios from 'axios';
import { AppDispatch } from '../store/index';
import { getPublicKey, useRegistrationCode, pingAll } from '../store/actions';
import { ThunkAction } from 'redux-thunk';
import { Action } from 'redux';
import { RootState } from '../store';
import { AppThunk } from '../types';
import { ShinkaiMessageBuilderWrapper } from '../lib/wasm/ShinkaiMessageBuilderWrapper';

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

export const submitRegistrationCode = (my_encryption_sk: string, my_signature_sk: string, receiver_public_key: string, code: string, identity_type: string, sender: string, receiver: string): AppThunk => async (dispatch: AppDispatch) => {
  try {
    // Build the ShinkaiMessage
    const messageStr = ShinkaiMessageBuilderWrapper.code_registration(my_encryption_sk, my_signature_sk, receiver_public_key, code, identity_type, sender, receiver);

    // Parse the message into a JSON object
    const message = JSON.parse(messageStr);

    // POST the message to the server
    await axios.post(`${API_URL}/use_registration_code`, message);

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
