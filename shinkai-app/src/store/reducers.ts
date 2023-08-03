import { Action, GET_PUBLIC_KEY, USE_REGISTRATION_CODE, PING_ALL } from './types';

export interface RootState {
  publicKey: string,
  registrationStatus: boolean,
  pingResult: string
}

const initialState: RootState = {
  publicKey: '',
  registrationStatus: false,
  pingResult: ''
};

const rootReducer = (state = initialState, action: Action): RootState => {
  switch (action.type) {
    case GET_PUBLIC_KEY:
      return { ...state, publicKey: action.payload };
    case USE_REGISTRATION_CODE:
      return { ...state, registrationStatus: true };
    case PING_ALL:
      return { ...state, pingResult: action.payload };
    default:
      return state;
  }
};

export default rootReducer;