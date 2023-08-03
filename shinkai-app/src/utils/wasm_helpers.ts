
import { generateKeyPair } from 'curve25519-js';
import * as ed from '@noble/ed25519';
import bs58 from 'bs58';

type Base58String = string;

export const generateEncryptionKeys = async (seed: Uint8Array): Promise<{my_encryption_sk_string: Base58String, my_encryption_pk_string: Base58String}> => {
  const encryptionKeys = generateKeyPair(seed);
  const my_encryption_sk_string: Base58String = bs58.encode(new Uint8Array(encryptionKeys.private));
  const my_encryption_pk_string: Base58String = bs58.encode(new Uint8Array(encryptionKeys.public));

  return {
    my_encryption_sk_string,
    my_encryption_pk_string,
  }
}

export const generateSignatureKeys = async (): Promise<{my_identity_sk_string: Base58String, my_identity_pk_string: Base58String}> => {
  const privKey = ed.utils.randomPrivateKey(); // Secure random private key
  const pubKey = await ed.getPublicKeyAsync(privKey);

  const my_identity_sk_string: Base58String = bs58.encode(new Uint8Array(privKey));
  const my_identity_pk_string: Base58String = bs58.encode(new Uint8Array(pubKey));

  return {
    my_identity_sk_string,
    my_identity_pk_string,
  }
}

export const test_util_generateKeys = async (): Promise<{my_encryption_sk_string: Base58String, my_encryption_pk_string: Base58String, receiver_public_key_string: Base58String, my_identity_sk_string: Base58String, my_identity_pk_string: Base58String}> => {
  const seed = new Uint8Array(32);

  const encryptionKeys = await generateEncryptionKeys(seed);
  const signatureKeys = await generateSignatureKeys();

  return {
    ...encryptionKeys,
    receiver_public_key_string: encryptionKeys.my_encryption_pk_string,
    ...signatureKeys
  }
}

export function mapEncryptionMethod(encryption: String): number {
    switch (encryption) {
      case "DiffieHellmanChaChaPoly1305":
        return 0;
      case "None":
        return 1;
      default:
        throw new Error("Unknown encryption method");
    }
  }
  