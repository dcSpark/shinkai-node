use std::cmp::Ordering;

#[derive(Clone, PartialEq, Debug)]
pub struct TokenNode {
    pub orig_pos: usize,
    pub token_id: usize,
    pub prev: Option<Box<TokenNode>>,
    pub next: Option<Box<TokenNode>>,
    pub merge_prio: f64,
    pub merge_to_string: String,
    pub deleted: bool,
}

impl TokenNode {
    pub fn new(orig_pos: usize, token_id: usize, prev: Option<TokenNode>, next: Option<TokenNode>) -> Self {
        TokenNode {
            orig_pos,
            token_id,
            prev: prev.map(Box::new),
            next: next.map(Box::new),
            merge_prio: 0.0,
            merge_to_string: String::new(),
            deleted: false,
        }
    }
}

impl Ord for TokenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.token_id.cmp(&other.token_id)
    }
}

impl PartialOrd for TokenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for TokenNode {}