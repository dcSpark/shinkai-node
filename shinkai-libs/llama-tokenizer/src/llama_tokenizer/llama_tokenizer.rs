use crate::llama_tokenizer::merge_binary::MERGE_BINARY;
use crate::llama_tokenizer::priority_queue::PriorityQueue;
use crate::llama_tokenizer::token_node::TokenNode;
use crate::llama_tokenizer::vocab::VOCAB_BASE64;
use base64;
use std::collections::HashMap;
use std::str;
use std::time::Instant;

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

    pub fn decode_vocabulary(&self, vocab_base64: &str) -> Vec<String> {
        let byte_array = self.base64decode(vocab_base64);
        let decoded_string = str::from_utf8(&byte_array).unwrap();
        decoded_string.split("\n").map(|s| s.to_string()).collect()
    }

    pub fn utf8_byte_to_hex(&self, c: u8) -> String {
        format!("<0x{:02X}>", c)
    }

    pub fn hex_to_utf8_byte(&self, hex: &str) -> u8 {
        let stripped_hex = hex.trim_start_matches("<0x").trim_end_matches(">");
        u8::from_str_radix(stripped_hex, 16).unwrap()
    }

    pub fn map_characters_to_token_ids(
        &self,
        mut prompt: String,
        add_bos_token: bool,
        add_preceding_space: bool,
    ) -> Vec<usize> {
        let mut token_ids = Vec::new();
        if add_bos_token {
            token_ids.push(1);
        }
        if add_preceding_space {
            prompt.insert(0, ' ');
        }
        let prompt_altered = prompt.replace(" ", &self.vocab_by_id[29871]);
        let char_array: Vec<char> = prompt_altered.chars().collect();
        for c in char_array {
            if let Some(token_id) = self.vocab_by_string.get(&c.to_string()) {
                token_ids.push(*token_id);
            } else {
                let bytes = c.to_string().into_bytes();
                for byte in bytes {
                    let hex = self.utf8_byte_to_hex(byte as u8);
                    if let Some(token_id) = self.vocab_by_string.get(&hex) {
                        token_ids.push(*token_id);
                    } else {
                        println!(
                            "Encountered unknown character {} (partial UTF-8 byte {} + hex {})",
                            c, byte, hex
                        );
                        token_ids.push(0);
                    }
                }
            }
        }
        token_ids
    }

    pub fn encode(
        &mut self,
        mut prompt: String,
        add_bos_token: bool,
        add_preceding_space: bool,
        log_performance: bool,
    ) -> Vec<usize> {
        let start_time = Instant::now();

        if self.vocab_by_id.is_empty() || self.vocab_by_string.is_empty() || self.merges.is_empty() {
            println!("Tokenizer not initialized properly!");
            return vec![];
        }
        if prompt.is_empty() {
            return vec![];
        }

        let mut token_ids = self.map_characters_to_token_ids(prompt, add_bos_token, add_preceding_space);

        let mut merge_queue = PriorityQueue::new();
        let mut first_token_node = TokenNode::new(0, token_ids[0], None, None);
        let mut prev_token_node = first_token_node.clone();

        for i in 1..token_ids.len() {
            let curr_token_node = TokenNode::new(i, token_ids[i], Some(prev_token_node.clone()), None);
            prev_token_node.next = Some(Box::new(curr_token_node.clone()));
            self.add_to_merge_queue(&prev_token_node, &mut merge_queue);
            prev_token_node = curr_token_node;
        }

        while !merge_queue.is_empty() {
            let mut left_of_merge = merge_queue.pop().unwrap();
            if left_of_merge.deleted || left_of_merge.next.is_none() || left_of_merge.next.as_ref().unwrap().deleted {
                continue;
            }

            left_of_merge.deleted = true;
            left_of_merge.next.as_mut().unwrap().deleted = true;

            if let Some(ref mut old_prev) = left_of_merge.prev {
                old_prev.deleted = true;
                let mut new_prev = old_prev.clone();
                let new_prev_clone = new_prev.clone();
                left_of_merge.prev = Some(Box::new((*new_prev_clone).clone()));
                if let Some(ref mut prev_of_prev) = new_prev.prev {
                    prev_of_prev.next = Some(Box::new((*new_prev_clone).clone()));
                } else {
                    first_token_node = (*new_prev_clone).clone();
                }
            }

            let merge_to_string = left_of_merge.merge_to_string.replace(" ", "");
            let token_id = *self.vocab_by_string.get(&merge_to_string).unwrap();
            let prev = left_of_merge.prev.clone().map(|node| *node.clone());
            let next = left_of_merge
                .next
                .as_ref()
                .unwrap()
                .next
                .clone()
                .map(|node| *node.clone());

            let mut result_of_merge = TokenNode::new(left_of_merge.orig_pos, token_id, prev, next);

            let result_of_merge_clone = result_of_merge.clone();

            if let Some(ref mut prev) = result_of_merge.prev {
                prev.next = Some(Box::new(result_of_merge_clone.clone()));
                self.add_to_merge_queue(prev, &mut merge_queue);
            } else {
                first_token_node = result_of_merge_clone.clone();
            }

            if let Some(ref mut next) = result_of_merge.next {
                next.prev = Some(Box::new(result_of_merge_clone.clone()));
                self.add_to_merge_queue(&result_of_merge, &mut merge_queue);
            }
        }

        let mut merged_token_ids = Vec::new();
        let mut curr_token_node = Some(first_token_node);
        while let Some(node) = curr_token_node {
            merged_token_ids.push(node.token_id);
            curr_token_node = node.next.map(|node| *node);
        }

        if log_performance {
            let duration = start_time.elapsed();
            println!("Tokenizer running time: {:?}", duration);
        }

        merged_token_ids
    }

    fn add_to_merge_queue(&self, left_node: &TokenNode, merge_queue: &mut PriorityQueue<TokenNode>) {
        eprintln!("left_node: {:?}", left_node);
        eprintln!("left_node.next: {:?}", left_node.next);
        eprintln!("merge_queue: {:?}", merge_queue);

        if let Some(next_node) = left_node.next.as_ref() {
            let merge_identifier_string = self.get_merge_identifier_string(left_node.token_id, next_node.token_id);
            if let Some(merge_value) = self.merges.get(&merge_identifier_string) {
                let merge_prio = (*merge_value as f64) + left_node.orig_pos as f64 / self.vocab_by_id.len() as f64;
                let mut left_node_clone = left_node.clone();
                left_node_clone.merge_prio = merge_prio;
                left_node_clone.merge_to_string = merge_identifier_string.replace(" ", "");
                merge_queue.push(left_node_clone);
            }
        }
    }

    pub fn decode(&self, token_ids: Vec<usize>, add_bos_token: bool, add_preceding_space: bool) -> String {
        let mut utf8_byte_vals = Vec::new();
        let start_index = if add_bos_token { 1 } else { 0 };
        for i in start_index..token_ids.len() {
            let token_id = token_ids[i];
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
        let spaces_fixed = decoded_string.replace(&self.vocab_by_id[29871], " ");
        if add_preceding_space {
            spaces_fixed[1..].to_string()
        } else {
            spaces_fixed
        }
    }
}
