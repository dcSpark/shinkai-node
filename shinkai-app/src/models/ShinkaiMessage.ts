export enum EncryptionMethod {
  DiffieHellmanChaChaPoly1305 = "DiffieHellmanChaChaPoly1305",
  None = "None",
}

export enum MessageSchemaType {
  JobCreationSchema = "JobCreationSchema",
  JobMessageSchema = "JobMessageSchema",
  PreMessageSchema = "PreMessageSchema",
  TextContent = "TextContent",
  Empty = "",
}

export interface JobScope {
  buckets: string[];
  documents: string[];
}

export interface JobCreation {
  scope: JobScope;
}

export interface JobMessage {
  job_id: string;
  content: string;
}

export interface JobToolCall {
  tool_id: string;
  inputs: Record<string, string>;
}

export enum JobRecipient {
  SelfNode = "SelfNode",
  User = "User",
  ExternalIdentity = "ExternalIdentity",
}

export interface JobPreMessage {
  tool_calls: JobToolCall[];
  content: string;
  recipient: JobRecipient;
}

export interface InternalMetadata {
  sender_subidentity: string;
  recipient_subidentity: string;
  message_schema_type: MessageSchemaType;
  inbox: string;
  encryption: EncryptionMethod;
}

export interface ExternalMetadata {
  sender: string;
  recipient: string;
  scheduled_time: string;
  signature: string;
  other: string;
}

export interface Body {
  content: string;
  internal_metadata: InternalMetadata | null;
}

export interface ShinkaiMessage {
  body: Body | null;
  external_metadata: ExternalMetadata | null;
  encryption: EncryptionMethod;
}

export interface RegistrationCode {
  code: string;
  profileName: string;
  identityPk: string;
  encryptionPk: string;
  permissionType: string;
}
