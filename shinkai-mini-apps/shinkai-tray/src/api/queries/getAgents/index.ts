import { getProfileAgents } from "@shinkai_network/shinkai-message-ts/api";
import type { GetAgentsInput } from "./types";

export const getAgents = async ({
  sender,
  senderSubidentity,
  shinkaiIdentity,
  my_device_encryption_sk,
  my_device_identity_sk,
  node_encryption_pk,
}: GetAgentsInput) => {
  const result = await getProfileAgents(sender, senderSubidentity, shinkaiIdentity, {
    my_device_encryption_sk,
    my_device_identity_sk,
    node_encryption_pk,
  });
  return result;
};
