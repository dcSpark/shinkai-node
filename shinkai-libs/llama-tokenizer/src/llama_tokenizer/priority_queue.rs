#[derive(Debug)]
pub struct PriorityQueue<T: Ord> {
    heap: Vec<T>,
}

impl<T: Ord> PriorityQueue<T> {
    pub fn new() -> Self {
        PriorityQueue { heap: Vec::new() }
    }

    pub fn size(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn peek(&self) -> Option<&T> {
        self.heap.get(0)
    }

    pub fn push(&mut self, value: T) {
        self.heap.push(value);
        self.sift_up(self.size() - 1);
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let size = self.size() - 1;
            self.heap.swap(0, size);
            let popped_value = self.heap.pop();
            self.sift_down(0);
            popped_value
        }
    }

    fn parent(&self, index: usize) -> usize {
        (index - 1) / 2
    }

    fn left(&self, index: usize) -> usize {
        2 * index + 1
    }

    fn right(&self, index: usize) -> usize {
        2 * index + 2
    }

    fn sift_up(&mut self, index: usize) {
        let mut child = index;
        while child > 0 && self.heap[child] > self.heap[self.parent(child)] {
            let parent = self.parent(child);
            self.heap.swap(parent, child);
            child = parent;
        }
    }

    fn sift_down(&mut self, index: usize) {
        let mut parent = index;
        loop {
            let left = self.left(parent);
            let right = self.right(parent);
            let mut largest = parent;

            if left < self.size() && self.heap[left] > self.heap[largest] {
                largest = left;
            }
            if right < self.size() && self.heap[right] > self.heap[largest] {
                largest = right;
            }
            if largest == parent {
                break;
            }
            self.heap.swap(parent, largest);
            parent = largest;
        }
    }
}
