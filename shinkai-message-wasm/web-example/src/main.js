import * as wasm from './wbg.js';
import { generateKeyPair } from 'curve25519-js';
import * as ed25519 from '@noble/ed25519';
import bs58 from 'bs58';

async function run() {
    await init(); // Initialize the WASM module

    // Generate encryption keys using curve25519
    let encryptionKeys = generateKeyPair();
    let my_encryption_sk_string = bs58.encode(Buffer.from(encryptionKeys.private));
    let my_encryption_pk_string = bs58.encode(Buffer.from(encryptionKeys.public));

    // Generate signature keys using @noble/ed25519
    let identitySecretKey = ed25519.utils.randomPrivateKey(); // creates new random key each call
    let identityPublicKey = ed25519.getPublicKey(identitySecretKey);
    let my_identity_sk_string = bs58.encode(Buffer.from(identitySecretKey));
    let my_identity_pk_string = bs58.encode(Buffer.from(identityPublicKey));

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
