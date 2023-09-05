import { ShinkaiMessage } from "../../models/ShinkaiMessage";
import { calculateMessageHash } from "../../utils/shinkai_message_handler";
import {
  ADD_MESSAGE_TO_INBOX,
  Action,
  RECEIVE_ALL_INBOXES_FOR_PROFILE,
  RECEIVE_LAST_MESSAGES_FROM_INBOX,
  RECEIVE_LOAD_MORE_MESSAGES_FROM_INBOX,
} from "../types";

export interface MessagesState {
  inboxes: {
    [inboxId: string]: any[];
  };
  messageHashes: {
    [inboxId: string]: Set<string>;
  };
}

const messagesState: MessagesState = {
  inboxes: {},
  messageHashes: {},
};

interface InboxMessagesAction {
  type: typeof RECEIVE_LOAD_MORE_MESSAGES_FROM_INBOX | typeof RECEIVE_LAST_MESSAGES_FROM_INBOX;
  payload?: {
    inboxId: string;
    messages: ShinkaiMessage[];
  };
}

interface AddMessageAction {
  type: typeof ADD_MESSAGE_TO_INBOX;
  payload?: {
    inboxId: string;
    message: ShinkaiMessage;
  };
}

interface ReceiveAllInboxesAction {
  type: typeof RECEIVE_ALL_INBOXES_FOR_PROFILE;
  payload?: any; // Update this to the actual type of the payload
}

type MessagesAction =
  | InboxMessagesAction
  | AddMessageAction
  | ReceiveAllInboxesAction;

export const messagesReducer = (
  state = messagesState,
  action: MessagesAction
): MessagesState => {
  switch (action.type) {
    case RECEIVE_LOAD_MORE_MESSAGES_FROM_INBOX: {
      if (!action.payload) {
        return state;
      }
      const { inboxId, messages } = action.payload;
      const currentMessages = state.inboxes[inboxId] || [];
      const currentMessageHashes = state.messageHashes[inboxId] || new Set();

      const uniqueNewMessages = messages.filter((msg: ShinkaiMessage) => {
        const hash = calculateMessageHash(msg);
        if (currentMessageHashes.has(hash)) {
          return false;
        } else {
          currentMessageHashes.add(hash);
          return true;
        }
      });

      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [...currentMessages, ...uniqueNewMessages],
        },
        messageHashes: {
          ...state.messageHashes,
          [inboxId]: currentMessageHashes,
        },
      };
    }
    case RECEIVE_LAST_MESSAGES_FROM_INBOX: {
      if (!action.payload) {
        return state;
      }
      const { inboxId, messages } = action.payload;
      const currentMessages = state.inboxes[inboxId] || [];
      const currentMessageHashes = state.messageHashes[inboxId] || new Set();

      const uniqueNewMessages = messages.filter((msg: ShinkaiMessage) => {
        const hash = calculateMessageHash(msg);
        if (currentMessageHashes.has(hash)) {
          return false;
        } else {
          currentMessageHashes.add(hash);
          return true;
        }
      });

      return {
        ...state,
        inboxes: {
          ...state.inboxes,
          [inboxId]: [...currentMessages, ...uniqueNewMessages],
        },
        messageHashes: {
          ...state.messageHashes,
          [inboxId]: currentMessageHashes,
        },
      };
    }
    case ADD_MESSAGE_TO_INBOX: {
      if (!action.payload) {
        return state;
      }
      const { inboxId, message } = action.payload;
      const currentMessages = state.inboxes[inboxId] || [];
      const currentMessageHashes = state.messageHashes[inboxId] || new Set();

      const hash = calculateMessageHash(message);
      if (currentMessageHashes.has(hash)) {
        // If the message is a duplicate, don't add it
        return state;
      } else {
        // If the message is unique, add it to the inbox and the hash to the set
        currentMessageHashes.add(hash);
        return {
          ...state,
          inboxes: {
            ...state.inboxes,
            [inboxId]: [message, ...currentMessages],
          },
          messageHashes: {
            ...state.messageHashes,
            [inboxId]: currentMessageHashes,
          },
        };
      }
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
    default:
      return state;
  }
};
