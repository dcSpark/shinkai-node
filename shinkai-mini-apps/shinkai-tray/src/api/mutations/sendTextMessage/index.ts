import { sendTextMessageWithInbox } from "@shinkai_network/shinkai-message-ts/api";

import { SendMessageToInboxInput } from "./types";

export const sendMessageToInbox = async ({
  sender,
  receiver,
  message,
  inboxId,
  my_device_encryption_sk,
  my_device_identity_sk,
  node_encryption_pk,
}: SendMessageToInboxInput) => {
  return await sendTextMessageWithInbox(sender, "", receiver, message, inboxId, {
    my_device_encryption_sk,
    my_device_identity_sk,
    node_encryption_pk,
  });
};
