use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl Priority {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Priority::Low,
            2 => Priority::Normal,
            3 => Priority::High,
            4 => Priority::Critical,
            _ => Priority::Normal,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

#[derive(Debug, Clone)]
pub struct PriorityItem<T> {
    pub priority: Priority,
    pub sequence: u64,
    pub item: T,
}

impl<T> PriorityItem<T> {
    pub fn new(item: T, priority: Priority, sequence: u64) -> Self {
        Self {
            priority,
            sequence,
            item,
        }
    }
}

impl<T> PartialEq for PriorityItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.sequence == other.sequence
    }
}

impl<T> Eq for PriorityItem<T> {}

impl<T> PartialOrd for PriorityItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for PriorityItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.sequence.cmp(&self.sequence),
            other => other,
        }
    }
}

#[derive(Debug)]
struct PriorityQueueInner<T> {
    heap: BinaryHeap<PriorityItem<T>>,
    sequence: u64,
}

impl<T> PriorityQueueInner<T> {
    fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            sequence: 0,
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
            sequence: 0,
        }
    }

    fn enqueue(&mut self, item: T, priority: Priority) {
        let priority_item = PriorityItem::new(item, priority, self.sequence);
        self.sequence = self.sequence.wrapping_add(1);
        self.heap.push(priority_item);
    }

    fn dequeue(&mut self) -> Option<PriorityItem<T>> {
        self.heap.pop()
    }

    fn peek(&self) -> Option<&PriorityItem<T>> {
        self.heap.peek()
    }

    fn len(&self) -> usize {
        self.heap.len()
    }

    fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    fn clear(&mut self) {
        self.heap.clear();
        self.sequence = 0;
    }

    fn capacity(&self) -> usize {
        self.heap.capacity()
    }
}

#[derive(Debug, Clone)]
pub struct PriorityQueue<T> {
    inner: Arc<Mutex<PriorityQueueInner<T>>>,
}

impl<T> PriorityQueue<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PriorityQueueInner::new())),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PriorityQueueInner::with_capacity(capacity))),
        }
    }

    pub fn enqueue(&self, item: T, priority: Priority) {
        let mut inner = self.inner.lock().unwrap();
        inner.enqueue(item, priority);
    }

    pub fn dequeue(&self) -> Option<PriorityItem<T>> {
        let mut inner = self.inner.lock().unwrap();
        inner.dequeue()
    }

    pub fn peek(&self) -> Option<Priority> {
        let inner = self.inner.lock().unwrap();
        inner.peek().map(|item| item.priority)
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.len()
    }

    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.is_empty()
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.clear();
    }

    pub fn capacity(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.capacity()
    }

    pub fn try_dequeue(&self) -> Option<PriorityItem<T>> {
        if let Ok(mut inner) = self.inner.try_lock() {
            inner.dequeue()
        } else {
            None
        }
    }

    pub fn drain(&self) -> Vec<PriorityItem<T>> {
        let mut inner = self.inner.lock().unwrap();
        let mut items = Vec::with_capacity(inner.len());
        while let Some(item) = inner.dequeue() {
            items.push(item);
        }
        items
    }

    pub fn count_by_priority(&self, priority: Priority) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.heap.iter().filter(|item| item.priority == priority).count()
    }
}

impl<T> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_queue_creation() {
        let queue: PriorityQueue<i32> = PriorityQueue::new();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_priority_ordering() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Low);
        queue.enqueue(2, Priority::High);
        queue.enqueue(3, Priority::Normal);
        queue.enqueue(4, Priority::Critical);
        
        assert_eq!(queue.dequeue().unwrap().priority, Priority::Critical);
        assert_eq!(queue.dequeue().unwrap().priority, Priority::High);
        assert_eq!(queue.dequeue().unwrap().priority, Priority::Normal);
        assert_eq!(queue.dequeue().unwrap().priority, Priority::Low);
    }

    #[test]
    fn test_fifo_within_priority() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Normal);
        queue.enqueue(2, Priority::Normal);
        queue.enqueue(3, Priority::Normal);
        
        assert_eq!(queue.dequeue().unwrap().item, 1);
        assert_eq!(queue.dequeue().unwrap().item, 2);
        assert_eq!(queue.dequeue().unwrap().item, 3);
    }

    #[test]
    fn test_enqueue_dequeue() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(42, Priority::Normal);
        assert_eq!(queue.len(), 1);
        
        let item = queue.dequeue().unwrap();
        assert_eq!(item.item, 42);
        assert_eq!(item.priority, Priority::Normal);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_peek() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Low);
        queue.enqueue(2, Priority::High);
        
        assert_eq!(queue.peek(), Some(Priority::High));
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_clear() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Normal);
        queue.enqueue(2, Priority::Normal);
        queue.enqueue(3, Priority::Normal);
        
        assert_eq!(queue.len(), 3);
        queue.clear();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_with_capacity() {
        let queue: PriorityQueue<i32> = PriorityQueue::with_capacity(100);
        assert!(queue.capacity() >= 100);
    }

    #[test]
    fn test_drain() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Low);
        queue.enqueue(2, Priority::High);
        queue.enqueue(3, Priority::Normal);
        
        let items = queue.drain();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].priority, Priority::High);
        assert_eq!(items[1].priority, Priority::Normal);
        assert_eq!(items[2].priority, Priority::Low);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_count_by_priority() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(1, Priority::Low);
        queue.enqueue(2, Priority::High);
        queue.enqueue(3, Priority::High);
        queue.enqueue(4, Priority::Normal);
        
        assert_eq!(queue.count_by_priority(Priority::High), 2);
        assert_eq!(queue.count_by_priority(Priority::Low), 1);
        assert_eq!(queue.count_by_priority(Priority::Normal), 1);
        assert_eq!(queue.count_by_priority(Priority::Critical), 0);
    }

    #[test]
    fn test_concurrent_enqueue() {
        use std::thread;
        
        let queue = PriorityQueue::new();
        let mut handles = vec![];
        
        for i in 0..10 {
            let queue_clone = queue.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    queue_clone.enqueue(i * 100 + j, Priority::Normal);
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(queue.len(), 1000);
    }

    #[test]
    fn test_concurrent_dequeue() {
        use std::thread;
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let queue = PriorityQueue::new();
        
        for i in 0..1000 {
            queue.enqueue(i, Priority::Normal);
        }
        
        let dequeued_count = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        
        for _ in 0..10 {
            let queue_clone = queue.clone();
            let count_clone = dequeued_count.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    if queue_clone.dequeue().is_some() {
                        count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(dequeued_count.load(Ordering::SeqCst), 1000);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_mixed_concurrent_operations() {
        use std::thread;
        
        let queue = PriorityQueue::new();
        let mut handles = vec![];
        
        for i in 0..5 {
            let queue_clone = queue.clone();
            let handle = thread::spawn(move || {
                for j in 0..50 {
                    queue_clone.enqueue(i * 50 + j, Priority::Normal);
                }
            });
            handles.push(handle);
        }
        
        for _ in 0..5 {
            let queue_clone = queue.clone();
            let handle = thread::spawn(move || {
                for _ in 0..50 {
                    queue_clone.dequeue();
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(Priority::from_u8(1), Priority::Low);
        assert_eq!(Priority::from_u8(2), Priority::Normal);
        assert_eq!(Priority::from_u8(3), Priority::High);
        assert_eq!(Priority::from_u8(4), Priority::Critical);
        assert_eq!(Priority::from_u8(99), Priority::Normal);
    }

    #[test]
    fn test_priority_as_u8() {
        assert_eq!(Priority::Low.as_u8(), 1);
        assert_eq!(Priority::Normal.as_u8(), 2);
        assert_eq!(Priority::High.as_u8(), 3);
        assert_eq!(Priority::Critical.as_u8(), 4);
    }

    #[test]
    fn test_try_dequeue() {
        let queue = PriorityQueue::new();
        
        queue.enqueue(42, Priority::Normal);
        
        let item = queue.try_dequeue();
        assert!(item.is_some());
        assert_eq!(item.unwrap().item, 42);
    }

    #[test]
    fn test_empty_dequeue() {
        let queue: PriorityQueue<i32> = PriorityQueue::new();
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn test_sequence_ordering() {
        let queue = PriorityQueue::new();
        
        for i in 0..100 {
            queue.enqueue(i, Priority::Normal);
        }
        
        for i in 0..100 {
            let item = queue.dequeue().unwrap();
            assert_eq!(item.item, i);
        }
    }
}
