import * as wasm from './pkg/shinkai_message_wasm.js';
import { sha512 } from '@noble/hashes/sha512';
import { generateKeyPair } from 'curve25519-js';
import bs58 from 'bs58';
import * as ed from '@noble/ed25519';

ed.etc.sha512Sync = (...m) => sha512(ed.etc.concatBytes(...m));

async function run() {
    // await wasm_bindgen('./shinkai_message_wasm_bg.wasm');

    // Generate encryption keys using curve25519
    const seed = window.crypto.getRandomValues(new Uint8Array(32));
    let encryptionKeys = generateKeyPair(seed);
    let my_encryption_sk_string = bs58.encode(new Uint8Array(encryptionKeys.private));
    let my_encryption_pk_string = bs58.encode(new Uint8Array(encryptionKeys.public));

    console.log("my_encryption_sk_string: ", my_encryption_sk_string);
    console.log("my_encryption_pk_string: ", my_encryption_pk_string);

    // Generate signature keys using ed25519
    const privKey = ed.utils.randomPrivateKey(); // Secure random private key
    const pubKey = await ed.getPublicKeyAsync(privKey); 

    let my_identity_sk_string = bs58.encode(new Uint8Array(privKey));
    let my_identity_pk_string = bs58.encode(new Uint8Array(pubKey));

    console.log("my_identity_sk_string: ", my_identity_sk_string);
    console.log("my_identity_pk_string: ", my_identity_pk_string);

    // Generate receiver's public key (for testing, we use our own public key)
    let receiver_public_key_string = my_encryption_pk_string;

    let sender_node = "@@sender_node.shinkai";
    let receiver_node = "@@receiver_node.shinkai";

    // Create a new ACK message
    let ack_message = wasm.ShinkaiMessageBuilderWrapper.ack_message(
        my_encryption_sk_string,
        my_identity_sk_string,
        receiver_public_key_string,
        sender_node,
        receiver_node
    );

    console.log(ack_message);
}

run();
