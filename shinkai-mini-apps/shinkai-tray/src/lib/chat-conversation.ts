import type { ShinkaiMessage } from "@shinkai_network/shinkai-message-ts/models";

export const groupMessagesByDate = (messages: ShinkaiMessage[]) => {
  const groupedMessages: Record<string, ShinkaiMessage[]> = {};
  for (const message of messages) {
    const date = new Date(message.external_metadata?.scheduled_time ?? "").toDateString();
    if (!groupedMessages[date]) {
      groupedMessages[date] = [];
    }
    groupedMessages[date].push(message);
  }
  return groupedMessages;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const getMessageFromJob = (message: any) => {
  if ("unencrypted" in message.body) {
    return JSON.parse(
      message.body.unencrypted.message_data.unencrypted.message_raw_content
    ).content;
  }
  return message.body.unencrypted.message_data.encrypted.content;
};
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const getMessageFromChat = (message: any) => {
  return message.body.unencrypted.message_data.unencrypted.message_raw_content;
};
