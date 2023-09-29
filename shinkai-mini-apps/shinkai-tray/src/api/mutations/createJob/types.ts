import type { JobCredentialsPayload } from "@shinkai_network/shinkai-message-ts/models";

export type CreateJobInput = JobCredentialsPayload & {
  shinkaiIdentity: string;
  profile: string;
  agentId: string;
  content: string;
};

export type CreateJobOutput = {
  jobId: string;
  response: string;
};
