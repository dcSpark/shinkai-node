/* tslint:disable */
/* eslint-disable */
/**
* @param {string} encryption_sk
* @returns {string}
*/
export function convert_encryption_sk_string_to_encryption_pk_string(encryption_sk: string): string;
/**
*/
export enum EncryptionMethod {
  DiffieHellmanChaChaPoly1305 = 0,
  None = 1,
}
/**
*/
export class ShinkaiMessageBuilderWrapper {
  free(): void;
/**
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
*/
  constructor(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string);
/**
* @param {any} encryption
*/
  body_encryption(encryption: any): void;
/**
*/
  no_body_encryption(): void;
/**
* @param {string} content
*/
  body(content: string): void;
/**
* @param {any} content
*/
  message_schema_type(content: any): void;
/**
* @param {string} sender_subidentity
* @param {string} recipient_subidentity
* @param {any} encryption
*/
  internal_metadata(sender_subidentity: string, recipient_subidentity: string, encryption: any): void;
/**
* @param {string} sender_subidentity
* @param {string} recipient_subidentity
* @param {string} inbox
* @param {any} encryption
*/
  internal_metadata_with_inbox(sender_subidentity: string, recipient_subidentity: string, inbox: string, encryption: any): void;
/**
* @param {string} sender_subidentity
* @param {string} recipient_subidentity
* @param {string} inbox
* @param {any} message_schema
* @param {any} encryption
*/
  internal_metadata_with_schema(sender_subidentity: string, recipient_subidentity: string, inbox: string, message_schema: any, encryption: any): void;
/**
*/
  empty_encrypted_internal_metadata(): void;
/**
*/
  empty_non_encrypted_internal_metadata(): void;
/**
* @param {string} recipient
* @param {string} sender
*/
  external_metadata(recipient: string, sender: string): void;
/**
* @param {string} recipient
* @param {string} sender
* @param {string} other
*/
  external_metadata_with_other(recipient: string, sender: string, other: string): void;
/**
* @param {string} recipient
* @param {string} sender
* @param {string} scheduled_time
*/
  external_metadata_with_schedule(recipient: string, sender: string, scheduled_time: string): void;
/**
* @returns {ShinkaiMessageWrapper}
*/
  build(): ShinkaiMessageWrapper;
/**
* @returns {any}
*/
  build_to_jsvalue(): any;
/**
* @returns {string}
*/
  build_to_string(): string;
/**
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {string} sender
* @param {string} receiver
* @returns {string}
*/
  static ack_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} permissions
* @param {string} code_type
* @param {string} sender_profile_name
* @param {string} receiver
* @returns {string}
*/
  static request_code_registration(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, permissions: string, code_type: string, sender_profile_name: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} code
* @param {string} identity_type
* @param {string} permission_type
* @param {string} registration_name
* @param {string} sender_profile_name
* @param {string} receiver
* @returns {string}
*/
  static use_code_registration(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, code: string, identity_type: string, permission_type: string, registration_name: string, sender_profile_name: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} inbox
* @param {number} count
* @param {string | undefined} offset
* @param {string} sender_profile_name
* @param {string} receiver
* @returns {string}
*/
  static get_last_messages_from_inbox(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, inbox: string, count: number, offset: string | undefined, sender_profile_name: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} inbox
* @param {number} count
* @param {string | undefined} offset
* @param {string} sender_profile_name
* @param {string} receiver
* @returns {string}
*/
  static get_last_unread_messages_from_inbox(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, inbox: string, count: number, offset: string | undefined, sender_profile_name: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} inbox
* @param {string} up_to_time
* @param {string} sender_profile_name
* @param {string} receiver
* @returns {string}
*/
  static read_up_to_time(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, inbox: string, up_to_time: string, sender_profile_name: string, receiver: string): string;
/**
* @param {string} my_subidentity_encryption_sk
* @param {string} my_subidentity_signature_sk
* @param {string} receiver_public_key
* @param {string} data
* @param {string} sender_profile_name
* @param {string} receiver
* @param {string} schema
* @returns {string}
*/
  static create_custom_shinkai_message_to_node(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, data: string, sender_profile_name: string, receiver: string, schema: string): string;
/**
* @param {string} message
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {string} sender
* @param {string} receiver
* @returns {string}
*/
  static ping_pong_message(message: string, my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string;
/**
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {any} scope
* @param {string} sender
* @param {string} receiver
* @param {string} receiver_subidentity
* @returns {string}
*/
  static job_creation(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, scope: any, sender: string, receiver: string, receiver_subidentity: string): string;
/**
* @param {string} job_id
* @param {string} content
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {string} sender
* @param {string} receiver
* @param {string} receiver_subidentity
* @returns {string}
*/
  static job_message(job_id: string, content: string, my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string, receiver_subidentity: string): string;
/**
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {string} sender
* @param {string} receiver
* @returns {string}
*/
  static terminate_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string;
/**
* @param {string} my_encryption_secret_key
* @param {string} my_signature_secret_key
* @param {string} receiver_public_key
* @param {string} sender
* @param {string} receiver
* @param {string} error_msg
* @returns {string}
*/
  static error_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string, error_msg: string): string;
}
/**
*/
export class ShinkaiMessageWrapper {
  free(): void;
/**
* @param {any} body
* @param {any} external_metadata
* @param {number} encryption
*/
  constructor(body: any, external_metadata: any, encryption: number);
/**
* @returns {any}
*/
  to_jsvalue(): any;
/**
* @param {any} j
* @returns {ShinkaiMessageWrapper}
*/
  static fromJsValue(j: any): ShinkaiMessageWrapper;
/**
* @returns {string}
*/
  to_json_str(): string;
/**
* @param {string} s
* @returns {ShinkaiMessageWrapper}
*/
  static from_json_str(s: string): ShinkaiMessageWrapper;
/**
* @returns {string}
*/
  calculate_hash(): string;
/**
* @returns {string}
*/
  static generate_time_now(): string;
/**
*/
  body: any;
/**
*/
  encryption: string;
/**
*/
  external_metadata: any;
}
