import { EncryptionMethod, ShinkaiMessageBuilderWrapper as ShinkaiMessageBuilderWrapperWASM, ShinkaiMessageWrapper } from '../../pkg/shinkai_message_wasm.js';
import { EncryptionMethod as TSEncryptionMethod } from '../../models/ShinkaiMessage.js';

export class ShinkaiMessageBuilderWrapper {
  private wasmBuilder: ShinkaiMessageBuilderWrapperWASM;

  constructor(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string) {
    this.wasmBuilder = new ShinkaiMessageBuilderWrapperWASM(my_encryption_secret_key, my_signature_secret_key, receiver_public_key);
  }

  body_encryption(encryption: TSEncryptionMethod): void {
    this.wasmBuilder.body_encryption(encryption);
  }

  no_body_encryption(): void {
    this.wasmBuilder.no_body_encryption();
  }

  body(content: string): void {
    this.wasmBuilder.body(content);
  }

  message_schema_type(content: any): void {
    this.wasmBuilder.message_schema_type(content);
  }

  internal_metadata(sender_subidentity: string, recipient_subidentity: string, inbox: string, encryption: any): void {
    this.wasmBuilder.internal_metadata(sender_subidentity, recipient_subidentity, inbox, encryption);
  }

  internal_metadata_with_schema(sender_subidentity: string, recipient_subidentity: string, inbox: string, message_schema: any, encryption: any): void {
    this.wasmBuilder.internal_metadata_with_schema(sender_subidentity, recipient_subidentity, inbox, message_schema, encryption);
  }

  empty_encrypted_internal_metadata(): void {
    this.wasmBuilder.empty_encrypted_internal_metadata();
  }

  empty_non_encrypted_internal_metadata(): void {
    this.wasmBuilder.empty_non_encrypted_internal_metadata();
  }

  external_metadata(recipient: string, sender: string): void {
    this.wasmBuilder.external_metadata(recipient, sender);
  }

  external_metadata_with_other(recipient: string, sender: string, other: string): void {
    this.wasmBuilder.external_metadata_with_other(recipient, sender, other);
  }

  external_metadata_with_schedule(recipient: string, sender: string, scheduled_time: string): void {
    this.wasmBuilder.external_metadata_with_schedule(recipient, sender, scheduled_time);
  }

  build(): ShinkaiMessageWrapper {
    const wasmWrapper = this.wasmBuilder.build();
    return ShinkaiMessageWrapper.fromJsValue(wasmWrapper.to_jsvalue());
  }

  build_to_jsvalue(): any {
    return this.wasmBuilder.build_to_jsvalue();
  }

  build_to_string(): string {
    return this.wasmBuilder.build_to_string();
  }

  static ack_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string {
    return ShinkaiMessageBuilderWrapperWASM.ack_message(my_encryption_secret_key, my_signature_secret_key, receiver_public_key, sender, receiver);
  }

  static code_registration(my_subidentity_encryption_sk: string, my_subidentity_signature_sk: string, receiver_public_key: string, code: string, identity_type: string, permission_type: string, registration_name: string, sender_profile_name: string, receiver: string): string {
    return ShinkaiMessageBuilderWrapperWASM.code_registration(my_subidentity_encryption_sk, my_subidentity_signature_sk, receiver_public_key, code, identity_type, permission_type, registration_name, sender_profile_name, receiver);
  }

  static ping_pong_message(message: string, my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string {
    return ShinkaiMessageBuilderWrapperWASM.ping_pong_message(message, my_encryption_secret_key, my_signature_secret_key, receiver_public_key, sender, receiver);
  }

  static job_creation(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, scope: any, sender: string, receiver: string, receiver_subidentity: string): string {
    return ShinkaiMessageBuilderWrapperWASM.job_creation(my_encryption_secret_key, my_signature_secret_key, receiver_public_key, scope, sender, receiver, receiver_subidentity);
  }

  static job_message(job_id: string, content: string, my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string, receiver_subidentity: string): string {
    return ShinkaiMessageBuilderWrapperWASM.job_message(job_id, content, my_encryption_secret_key, my_signature_secret_key, receiver_public_key, sender, receiver, receiver_subidentity);
  }

  static terminate_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string): string {
    return ShinkaiMessageBuilderWrapperWASM.terminate_message(my_encryption_secret_key, my_signature_secret_key, receiver_public_key, sender, receiver);
  }

  static error_message(my_encryption_secret_key: string, my_signature_secret_key: string, receiver_public_key: string, sender: string, receiver: string, error_msg: string): string {
    return ShinkaiMessageBuilderWrapperWASM.error_message(my_encryption_secret_key, my_signature_secret_key, receiver_public_key, sender, receiver, error_msg);
  }
}
