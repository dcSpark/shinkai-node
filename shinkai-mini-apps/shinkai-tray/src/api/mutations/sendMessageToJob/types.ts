import type { JobCredentialsPayload } from "@shinkai_network/shinkai-message-ts/models";

export type SendMessageToJobInput = JobCredentialsPayload & {
  jobId: string;
  message: string;
  sender: string;
  shinkaiIdentity: string;
};
