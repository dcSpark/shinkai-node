import { Base58String } from "../models/QRSetupData";
import { ShinkaiMessage } from "../models/ShinkaiMessage";
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
  RECEIVE_ALL_INBOXES_FOR_PROFILE,
  RECEIVE_LOAD_MORE_MESSAGES_FROM_INBOX,
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
    case RECEIVE_LOAD_MORE_MESSAGES_FROM_INBOX: {
      const { inboxId, messages } = action.payload;
      const currentMessages = state.inboxes[inboxId] || [];
      const lastCurrentMessageTimestamp =
        currentMessages.length > 0 &&
        currentMessages[currentMessages.length - 1].external_metadata
          ? new Date(
              currentMessages[
                currentMessages.length - 1
              ].external_metadata.scheduled_time
            )
          : null;
      const lastNewMessageTimestamp = messages[messages.length - 1]
        .external_metadata
        ? new Date(
            messages[messages.length - 1].external_metadata.scheduled_time
          )
        : null;

      const newMessages =
        currentMessages.length === 0 ||
        (lastCurrentMessageTimestamp &&
          lastNewMessageTimestamp &&
          lastCurrentMessageTimestamp.getTime() <
            lastNewMessageTimestamp.getTime())
          ? messages
          : [];

      console.log(
        "last current messages: ",
        currentMessages[currentMessages.length - 1]
      );
      console.log("last new messages: ", messages[messages.length - 1]);

      console.log("newMessages: ", newMessages);
      console.log("currentMessages: ", currentMessages);
      console.log("messages: ", messages);

      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [...newMessages, ...currentMessages],
        },
      };
    }
    case RECEIVE_LAST_MESSAGES_FROM_INBOX: {
      const { inboxId, messages } = action.payload;
      const currentMessages = state.inboxes[inboxId] || [];
      const lastMessageTimestamp =
        currentMessages.length > 0 &&
        currentMessages[currentMessages.length - 1].external_metadata
          ? new Date(
              currentMessages[
                currentMessages.length - 1
              ].external_metadata.scheduled_time
            )
          : null;
      const firstNewMessageTimestamp = messages[0].external_metadata
        ? new Date(messages[0].external_metadata.scheduled_time)
        : null;

      const newMessages =
        currentMessages.length === 0 ||
        (lastMessageTimestamp &&
          firstNewMessageTimestamp &&
          firstNewMessageTimestamp > lastMessageTimestamp)
          ? messages
          : [];
      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [...currentMessages, ...newMessages],
        },
      };
    }
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
    case RECEIVE_ALL_INBOXES_FOR_PROFILE: {
      const newInboxes = action.payload;
      if (typeof newInboxes !== "object") {
        console.error(
          "Invalid payload for RECEIVE_ALL_INBOXES_FOR_PROFILE: ",
          newInboxes
        );
        return state;
      }
      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          ...Object.keys(newInboxes).reduce(
            (result: { [key: string]: any[] }, key) => {
              if (!state.inboxes[key]) {
                console.log("value for key: ", newInboxes[key]);
                result[newInboxes[key]] = [];
              }
              return result;
            },
            {}
          ),
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
