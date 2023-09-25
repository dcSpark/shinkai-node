import { ApiConfig, getProfileAgents } from "@shinkai_network/shinkai-message-ts/api";
import { GetAgents } from "./types";

export const getAgents = async ({
  sender,
  senderSubidentity,
  shinkaiIdentity,
  my_device_encryption_sk,
  my_device_identity_sk,
  node_encryption_pk,
}: GetAgents) => {
  ApiConfig.getInstance().setEndpoint("http://localhost:9550");
  const result = await getProfileAgents(sender, senderSubidentity, shinkaiIdentity, {
    my_device_encryption_sk,
    my_device_identity_sk,
    node_encryption_pk,
  });
  return result;
};
