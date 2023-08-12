import { test } from 'vitest'
import { ShinkaiMessageBuilderWrapper } from './ShinkaiMessageBuilderWrapper';
import { sha512 } from '@noble/hashes/sha512';
import { generateKeyPair } from 'curve25519-js';
import bs58 from 'bs58';
import * as ed from '@noble/ed25519';
import { EncryptionMethod, MessageSchemaType } from '../../models/ShinkaiMessage.js';

// Enable synchronous methods
ed.etc.sha512Sync = (...m) => sha512(ed.etc.concatBytes(...m));

const { Crypto } = require("@peculiar/webcrypto");
const crypto = new Crypto();
globalThis.crypto = crypto;

const generateKeys = async () => {
  const seed = new Uint8Array(32);
  let encryptionKeys = generateKeyPair(seed);
  let my_encryption_sk_string = bs58.encode(new Uint8Array(encryptionKeys.private));
  let my_encryption_pk_string = bs58.encode(new Uint8Array(encryptionKeys.public));

  const privKey = ed.utils.randomPrivateKey(); // Secure random private key
  const pubKey = await ed.getPublicKeyAsync(privKey); 

  let my_identity_sk_string = bs58.encode(new Uint8Array(privKey));
  let my_identity_pk_string = bs58.encode(new Uint8Array(pubKey));

  let receiver_public_key_string = my_encryption_pk_string;

  return {
    my_encryption_sk_string,
    my_encryption_pk_string,
    my_identity_sk_string,
    my_identity_pk_string,
    receiver_public_key_string
  }
}

test('ShinkaiMessageBuilderWrapper should construct correctly and create a new ack message', async () => {
  const keys = await generateKeys();

  const messageBuilder = new ShinkaiMessageBuilderWrapper(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string
  );
  
  expect(messageBuilder).toBeTruthy();
  expect(messageBuilder).toBeInstanceOf(ShinkaiMessageBuilderWrapper);
  
  const sender = '@@sender_node.shinkai';
  const receiver = '@@receiver_node.shinkai';

  const ackMessage = ShinkaiMessageBuilderWrapper.ack_message(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string, 
    sender, 
    receiver
  );

  expect(ackMessage).toBeTruthy();
  expect(typeof ackMessage).toBe('string');
});

test('ShinkaiMessageBuilderWrapper should set body content correctly', async () => {
  const keys = await generateKeys();

  const messageBuilder = new ShinkaiMessageBuilderWrapper(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string
  );

  // Pass the enum value directly
  await messageBuilder.body('Hello world!');
  await messageBuilder.body_encryption(EncryptionMethod.None);
  await messageBuilder.message_schema_type(MessageSchemaType.TextContent);
  await messageBuilder.internal_metadata('sender_user2', 'recipient_user1', '', 'None');
  await messageBuilder.external_metadata_with_schedule('@@other_node.shinkai', '@@my_node.shinkai', '20230702T20533481345');

  const message = messageBuilder.build_to_string();

  expect(message).toContain('Hello world!');
});

test('ShinkaiMessageBuilderWrapper should create a use code registration message', async () => {
  const keys = await generateKeys();

  const registrationCode = 'sample_registration_code';
  const identityType = 'profile';
  const permissionType = 'admin';
  const registrationName = 'sample_registration_name';
  const shinkaiIdentity = '@@my_node.shinkai';

  const codeRegistrationMessage = ShinkaiMessageBuilderWrapper.use_code_registration(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string, 
    registrationCode,
    identityType,
    permissionType,
    registrationName,
    '', // sender_profile_name: it doesn't exist yet in the Node
    shinkaiIdentity
  );

  expect(codeRegistrationMessage).toBeTruthy();
  expect(typeof codeRegistrationMessage).toBe('string');
});

test('ShinkaiMessageBuilderWrapper should create a new request code registration message', async () => {
  const keys = await generateKeys();

  const messageBuilder = new ShinkaiMessageBuilderWrapper(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string
  );

  const permissionType = 'admin';
  const codeType = 'profile';
  const senderProfileName = 'sample_sender_profile_name';
  const shinkaiIdentity = '@@my_node.shinkai';

  const requestCodeRegistrationMessage = ShinkaiMessageBuilderWrapper.request_code_registration(
    keys.my_encryption_sk_string, 
    keys.my_identity_sk_string, 
    keys.receiver_public_key_string, 
    permissionType,
    codeType,
    senderProfileName,
    shinkaiIdentity
  );

  expect(requestCodeRegistrationMessage).toBeTruthy();
  expect(typeof requestCodeRegistrationMessage).toBe('string');
});