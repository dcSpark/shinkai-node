import { Base58String } from "../models/QRSetupData";
import {
  Action,
  GET_PUBLIC_KEY,
  USE_REGISTRATION_CODE,
  PING_ALL,
  REGISTRATION_ERROR,
  CREATE_REGISTRATION_CODE,
  CLEAR_REGISTRATION_CODE,
  RECEIVE_LAST_MESSAGES_FROM_INBOX,
  CLEAR_STORE,
  ADD_MESSAGE_TO_INBOX,
} from "./types";

export type SetupDetailsState = {
  profile: string;
  permission_type: string;
  registration_name: string;
  node_address: string;
  shinkai_identity: string;
  node_encryption_pk: Base58String;
  node_signature_pk: Base58String;
  myEncryptionSk: Base58String;
  myEncryptionPk: Base58String;
  myIdentitySk: Base58String;
  myIdentityPk: Base58String;
};

const setupInitialState: SetupDetailsState = {
  profile: "",
  permission_type: "",
  registration_name: "",
  node_address: "",
  shinkai_identity: "",
  node_encryption_pk: "",
  node_signature_pk: "",
  myEncryptionSk: "",
  myEncryptionPk: "",
  myIdentitySk: "",
  myIdentityPk: "",
};

export interface RootState {
  registrationCode: string;
  publicKey: string;
  registrationStatus: boolean;
  pingResult: string;
  setupDetailsState: SetupDetailsState;
  error: string | null;
  inboxes: {
    [inboxId: string]: any[];
  };
}

const initialState: RootState = {
  publicKey: "",
  registrationStatus: false,
  pingResult: "",
  setupDetailsState: setupInitialState,
  registrationCode: "",
  error: null,
  inboxes: {},
};

const rootReducer = (state = initialState, action: Action): RootState => {
  switch (action.type) {
    case GET_PUBLIC_KEY:
      return { ...state, publicKey: action.payload };
    case USE_REGISTRATION_CODE:
      return {
        ...state,
        registrationStatus: true,
        setupDetailsState: action.payload,
      };
    case RECEIVE_LAST_MESSAGES_FROM_INBOX:
      const { inboxId, messages } = action.payload;
      console.log("RECEIVE_LAST_MESSAGES_FROM_INBOX: ", inboxId, messages);
      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [...(state.inboxes[inboxId] || []), ...messages],
        },
      };
    case ADD_MESSAGE_TO_INBOX: {
      const { inboxId, message } = action.payload;
      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [message, ...(state.inboxes[inboxId] || [])],
        },
      };
    }
    case CREATE_REGISTRATION_CODE:
      return { ...state, registrationCode: action.payload };
    case REGISTRATION_ERROR:
      return { ...state, error: action.payload };
    case CLEAR_REGISTRATION_CODE:
      return { ...state, registrationCode: "" };
    case PING_ALL:
      return { ...state, pingResult: action.payload };
    case CLEAR_STORE:
      state = initialState;
      return state;
    default:
      return state;
  }
};

export default rootReducer;
