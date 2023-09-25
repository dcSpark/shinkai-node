import { ApiConfig, addAgent } from "@shinkai_network/shinkai-message-ts/api";
import { CreateAgentInput } from "./types";

export const createAgent = async (data: CreateAgentInput) => {
  const { sender_subidentity, node_name, agent, setupDetailsState } = data;
  ApiConfig.getInstance().setEndpoint("http://localhost:9550");

  return await addAgent(sender_subidentity, node_name, agent, setupDetailsState);
};
