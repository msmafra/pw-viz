#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Id(usize);

impl Id {
    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

pub struct IdAllocator {
    freed: Vec<usize>,
    next_id: usize,
}

impl IdAllocator {
    pub fn new() -> Self {
        Self {
            freed: Vec::new(),
            next_id: 0,
        }
    }
    pub fn allocate(&mut self) -> Id {
        let inner = if let Some(id) = self.freed.pop() {
            id
        } else {
            self.next_id += 1;

            self.next_id
        };

        Id(inner)
    }
    pub fn free(&mut self, id: Id) {
        self.freed.push(id.0);
    }
}
