import type {
  CredentialsPayload,
  ShinkaiMessage,
} from "@shinkai_network/shinkai-message-ts/models";

import { ApiConfig, handleHttpError } from "@shinkai_network/shinkai-message-ts/api";
import { ShinkaiMessageBuilderWrapper } from "@shinkai_network/shinkai-message-ts/wasm";

export type SmartInbox = {
  custom_name: string;
  inbox_id: string;
  last_message: ShinkaiMessage;
};

export const updateInboxNameApi = async (
  sender: string,
  sender_subidentity: string,
  receiver: string,
  target_shinkai_name_profile: string,
  setupDetailsState: CredentialsPayload
): Promise<SmartInbox[]> => {
  try {
    // TODO: Fix ShinkaiMessageBuilderWrapper
    const messageString = ShinkaiMessageBuilderWrapper.get_all_inboxes_for_profile(
      setupDetailsState.my_device_encryption_sk,
      setupDetailsState.my_device_identity_sk,
      setupDetailsState.node_encryption_pk,
      sender + "/" + sender_subidentity,
      "",
      receiver,
      target_shinkai_name_profile
    );

    const message = JSON.parse(messageString);

    const apiEndpoint = ApiConfig.getInstance().getEndpoint();
    const response = await fetch(`${apiEndpoint}/v1/update_smart_inbox_name`, {
      method: "POST",
      body: JSON.stringify(message),
      headers: { "Content-Type": "application/json" },
    });
    const data = await response.json();
    await handleHttpError(response);
    return data.data;
  } catch (error) {
    console.error("Error getting all inboxes for profile:", error);
    throw error;
  }
};

export const updateInboxName = async ({
  receiver,
  senderSubidentity,
  sender,
  targetShinkaiNameProfile,
  my_device_encryption_sk,
  my_device_identity_sk,
  node_encryption_pk,
  profile_encryption_sk,
  profile_identity_sk, // eslint-disable-next-line @typescript-eslint/no-explicit-any
}: any) => {
  const response = await updateInboxNameApi(
    sender,
    senderSubidentity,
    receiver,
    targetShinkaiNameProfile,
    {
      my_device_encryption_sk,
      my_device_identity_sk,
      node_encryption_pk,
      profile_encryption_sk,
      profile_identity_sk,
    }
  );
  console.log("updateInboxName response:", response);

  return response;
};
