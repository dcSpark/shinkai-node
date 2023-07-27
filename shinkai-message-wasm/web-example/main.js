import init, { create_message, parse_message } from './pkg/shinkai_message_wasm.js';

async function run() {
    await init(); // Initialize the WASM module

    // Create a new message
    let message_bytes = create_message();

    // Parse the message
    let shinkai_message = parse_message(message_bytes);

    console.log(shinkai_message);
}

run();
