import { getLastMessagesFromInbox } from "@shinkai_network/shinkai-message-ts/api";
import type { ShinkaiMessage } from "@shinkai_network/shinkai-message-ts/models";
import { GetChatConversationInput } from "./types";

export const getChatConversation = async ({
  inboxId,
  count,
  lastKey,
  shinkaiIdentity,
  profile,
  profile_encryption_sk,
  profile_identity_sk,
  node_encryption_pk,
}: GetChatConversationInput) => {
  const data: ShinkaiMessage[] = await getLastMessagesFromInbox(inboxId, count, lastKey, {
    shinkai_identity: shinkaiIdentity,
    profile: profile,
    profile_encryption_sk,
    profile_identity_sk,
    node_encryption_pk,
  });
  return data;
};
