import { getAllInboxesForProfile } from "@shinkai_network/shinkai-message-ts/api";

import type { GetInboxesInput } from "./types";

export const getInboxes = async ({
  receiver,
  senderSubidentity,
  shinkaiIdentity,
  targetShinkaiNameProfile,
  my_device_encryption_sk,
  my_device_identity_sk,
  node_encryption_pk,
}: GetInboxesInput) => {
  const inboxes = await getAllInboxesForProfile(
    shinkaiIdentity,
    senderSubidentity,
    receiver,
    targetShinkaiNameProfile,
    {
      my_device_encryption_sk,
      my_device_identity_sk,
      node_encryption_pk,
    }
  );
  return inboxes.map((inbox) => encodeURIComponent(inbox));
};
