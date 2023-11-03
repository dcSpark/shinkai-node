import { ApiConfig, handleHttpError } from "@shinkai_network/shinkai-message-ts/api";
import {
  type CredentialsPayload,
  MessageSchemaType,
  type ShinkaiMessage,
} from "@shinkai_network/shinkai-message-ts/models";
import { ShinkaiMessageBuilderWrapper } from "@shinkai_network/shinkai-message-ts/wasm";

export type SmartInbox = {
  custom_name: string;
  inbox_id: string;
  last_message: ShinkaiMessage;
};

function update_shinkai_inbox_name_wasm(
  my_encryption_secret_key: string,
  my_signature_secret_key: string,
  receiver_public_key: string,
  sender: string,
  sender_subidentity: string,
  receiver: string,
  receiver_subidentity: string,
  inbox: string,
  text_message: string
): string {
  const builder = new ShinkaiMessageBuilderWrapper(
    my_encryption_secret_key,
    my_signature_secret_key,
    receiver_public_key
  );

  builder.message_raw_content(text_message);
  builder.message_schema_type(MessageSchemaType.TextContent.toString());
  builder.internal_metadata_with_inbox(
    sender_subidentity,
    receiver_subidentity,
    inbox,
    "None"
  );
  builder.external_metadata_with_intra(receiver, sender, sender_subidentity);

  // TODO: At this point we are forcing unencrypted message until we implement message response in shinkai-node
  builder.body_encryption("None");

  const message = builder.build_to_string();

  return message;
}

export const updateInboxNameApi = async (
  sender: string,
  sender_subidentity: string,
  receiver: string,
  target_shinkai_name_profile: string,
  setupDetailsState: CredentialsPayload,
  inboxName: string,
  inbox: string
): Promise<SmartInbox[]> => {
  try {
    // TODO: Fix ShinkaiMessageBuilderWrapper
    const messageString = update_shinkai_inbox_name_wasm(
      setupDetailsState.my_device_encryption_sk,
      setupDetailsState.my_device_identity_sk,
      setupDetailsState.node_encryption_pk,
      sender + "/" + sender_subidentity,
      "",
      receiver,
      target_shinkai_name_profile,
      inboxName,
      inbox
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
    console.error("Error updating inbox name:", error);
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
  inboxName,
  inbox,
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
    },
    inboxName,
    inbox
  );
  console.log("updateInboxName response:", response);

  return response;
};
