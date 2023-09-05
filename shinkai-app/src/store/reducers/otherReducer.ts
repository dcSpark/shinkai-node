import { SerializedAgent } from "../../models/SchemaTypes";
import {
  ADD_AGENTS,
  Action,
  CLEAR_REGISTRATION_CODE,
  CLEAR_STORE,
  CREATE_REGISTRATION_CODE,
  GET_PUBLIC_KEY,
  PING_ALL,
  REGISTRATION_ERROR,
  USE_REGISTRATION_CODE,
} from "../types";

export interface OtherState {
  registrationCode: string;
  publicKey: string;
  registrationStatus: boolean;
  pingResult: string;
  error: string | null;
  agents: {
    [agentId: string]: SerializedAgent;
  };
}

const initialState: OtherState = {
  publicKey: "",
  registrationStatus: false,
  pingResult: "",
  registrationCode: "",
  error: null,
  agents: {},
};

const otherReducer = (state = initialState, action: Action): OtherState => {
  switch (action.type) {
    case USE_REGISTRATION_CODE:
      return {
        ...state,
        registrationStatus: true,
      };
    case GET_PUBLIC_KEY:
      return { ...state, publicKey: action.payload };
    case ADD_AGENTS: {
      const newAgents = action.payload;
      const updatedAgents = { ...state.agents };
      newAgents.forEach((agent: SerializedAgent) => {
        updatedAgents[agent.id] = agent;
      });
      return {
        ...state,
        agents: updatedAgents,
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

export default otherReducer;
