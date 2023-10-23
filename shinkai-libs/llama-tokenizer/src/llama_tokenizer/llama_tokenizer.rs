use crate::llama_tokenizer::merge_binary::MERGE_BINARY;
use crate::llama_tokenizer::vocab::VOCAB_BASE64;
use base64;
use std::collections::HashMap;
use std::str;

// Code ported from:
// https://github.com/danielgrittner/llama2-rs/blob/main/src/main.rs
// and https://github.com/belladoreai/llama-tokenizer-js

/**
 * MIT LICENSE
 * 
 * Copyright 2023 belladore.ai
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 * 
 */

/**
 * Helper function to decode the vocabulary.
 *
 * vocab_base64 is base64-encoded string of tokens delimited by '\n' (line break) in utf-8.
 * The row number of the token (indexing from 0) represents the id of the token in LLaMA tokenizer.
 *
 * Most tokens look like this: "ic" (without the quotes) (representing the "i" character followed by the "c" character)
 * Some tokens are special. In particular, spaces are replaced with the "▁" character and line-break is represented as "<0x0A>".
 *
 * This helper function returns the vocabulary as an array that contains Strings representing tokens:
 *
 *  "<unk>"   // Special token: unknown token
 *  "<s>"     // Special token: beginning of string
 *  "</s>"    // Special token: end of string
 *  "<0x00>"  // Byte-level token representing the 0-byte
 *  "<0x01>"  // Byte-level token ...
 *  "<0x02>"  // Byte-level token ...
 *  ...       // More byte-level tokens
 *  "<0x0A>"  // Byte-level token representing '\n' (line break). This is one of the few byte-level tokens that appear to be actually needed in practice.
 *  ...       // More byte-level tokens
 *  "<0xFF>"  // Byte-level token ...
 *  "▁▁"     // Token representing 2 consecutive spaces.
 *  "▁t"     // Token representing the space character followed by the "t" character.
 *  "er"      // Token representing the "e" character followed by the "r" character. Most tokens look like this.
 *  ...       // 32000 tokens
 */

pub struct LlamaTokenizer {
    pub vocab_by_id: Vec<String>,
    pub vocab_by_string: HashMap<String, usize>,
    pub merges: HashMap<String, usize>,
}

impl LlamaTokenizer {
    pub fn new() -> Self {
        let mut tokenizer = Self {
            vocab_by_id: Vec::new(),
            vocab_by_string: HashMap::new(),
            merges: HashMap::new(),
        };
        // Array where index represents tokenId, value represents tokenString
        tokenizer.vocab_by_id = tokenizer.decode_vocabulary(VOCAB_BASE64);
        // Map where key represents tokenString, value represents tokenId
        for (token_id, token_string) in tokenizer.vocab_by_id.iter().enumerate() {
            tokenizer.vocab_by_string.insert(token_string.clone(), token_id);
        }
        // Map where key identifies token pair, value represents merge priority
        tokenizer.merges = tokenizer.decompress_merges(MERGE_BINARY);
        tokenizer
    }

    // Equivalent of base64decode function
    pub fn base64decode(&self, encoded_string: &str) -> Vec<u8> {
        base64::decode(encoded_string).unwrap()
    }

    // Equivalent of getMergeIdentifierString function
    pub fn get_merge_identifier_string(&self, first_token_id: usize, second_token_id: usize) -> String {
        format!(
            "{} {}",
            self.vocab_by_id[first_token_id], self.vocab_by_id[second_token_id]
        )
    }

    pub fn num_to_vocab(&self, num: usize) -> &str {
        &self.vocab_by_id[num]
    }

    // Partial equivalent of decompressMerges function
    pub fn decompress_merges(&self, merges_binary: &str) -> HashMap<String, usize> {
        let byte_array_string = self.base64decode(merges_binary);
        let mut token_ids = Vec::new();
        for i in (0..byte_array_string.len()).step_by(2) {
            let byte1 = byte_array_string[i] as u16;
            let byte2 = (byte_array_string[i + 1] as u16) << 8;
            let token_id = byte1 + byte2;
            token_ids.push(token_id);
        }

        let mut merges = HashMap::new();
        for i in (0..token_ids.len()).step_by(2) {
            let id1 = token_ids[i];
            let id2 = token_ids[i + 1];
            let merge_identifier_string = self.get_merge_identifier_string(id1 as usize, id2 as usize);
            merges.insert(merge_identifier_string, i + 1);
        }
        merges
    }

    pub fn decode_vocabulary(&self, vocab_base64: &str) -> Vec<String> {
        let byte_array = self.base64decode(vocab_base64);
        let decoded_string = str::from_utf8(&byte_array).unwrap();
        decoded_string.split("\n").map(|s| s.to_string()).collect()
    }

    pub fn hex_to_utf8_byte(&self, hex: &str) -> u8 {
        let stripped_hex = hex.trim_start_matches("<0x").trim_end_matches(">");
        u8::from_str_radix(stripped_hex, 16).unwrap()
    }

    pub fn encode(&self, text: &str) -> Vec<usize> {
        let mut tokens = Vec::new();
        tokens.reserve(text.len());

        let text = text.replace(" ", "_").replace("\n", "<0x0A>");

        // encode every individual byte
        for ch in text.chars() {
            let token_id = self.vocab_by_string.get(ch.to_string().as_str()).unwrap_or(&0);
            tokens.push(*token_id);
        }

        let mut str_buffer = String::with_capacity(2 * self.vocab_by_id.len());

        // merge the best consecutive pair each iteration, according the scores in vocab_scores
        loop {
            let mut best_score = -1e10;
            let mut best_token_id = usize::MAX;
            let mut best_idx = usize::MAX;

            for i in 0..tokens.len() - 1 {
                // Copy the two consecutive tokens into a single string
                str_buffer.clear();
                str_buffer.push_str(&self.vocab_by_id[tokens[i]]);
                str_buffer.push_str(&self.vocab_by_id[tokens[i + 1]]);

                if let Some(token_id) = self.vocab_by_string.get(&str_buffer) {
                    let score = *self.merges.get(&str_buffer).unwrap_or(&0) as f32;
                    if score > best_score {
                        best_score = score;
                        best_token_id = *token_id;
                        best_idx = i;
                    }
                }
            }

            if best_idx == usize::MAX {
                break;
            }

            // Merge the best pair and delete the second token
            tokens[best_idx] = best_token_id;
            tokens.remove(best_idx + 1);
        }

        tokens
    }

    pub fn decode(&self, token_ids: Vec<usize>) -> String {
        let mut utf8_byte_vals = Vec::new();
        for token_id in token_ids {
            let token_string = &self.vocab_by_id[token_id];
            if token_string.starts_with("<0x") && token_string.ends_with(">") {
                let utf8_byte = self.hex_to_utf8_byte(token_string);
                utf8_byte_vals.push(utf8_byte);
            } else {
                let utf8_bytes = token_string.as_bytes();
                utf8_byte_vals.extend_from_slice(utf8_bytes);
            }
        }
        let decoded_string = str::from_utf8(&utf8_byte_vals).unwrap();
        decoded_string.replace(&self.vocab_by_id[29871], " ")
    }
}

