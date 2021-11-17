#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Id(usize);

pub struct IdAllocator {
    deallocated: Vec<usize>,
    next_id: usize
}

impl IdAllocator {
    pub fn new() -> Self {
        Self {
            deallocated: Vec::new(),
            next_id: 0
        }
    }
    pub fn allocate(&mut self) -> Id {
        let inner = if let Some(id) = self.deallocated.pop() {
            id
        } else {
            self.next_id+=1;

            self.next_id
        };

        Id(inner)
    }
    pub fn deallocate(&mut self, id: Id) {
        self.deallocated.push(id.0);
    } 
}