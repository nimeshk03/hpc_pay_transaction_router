use transaction_router::PriorityQueue;
use transaction_router::queue::priority_queue::Priority;

#[test]
fn test_basic_priority_ordering() {
    let queue = PriorityQueue::new();
    
    queue.enqueue("low", Priority::Low);
    queue.enqueue("high", Priority::High);
    queue.enqueue("normal", Priority::Normal);
    queue.enqueue("critical", Priority::Critical);
    
    assert_eq!(queue.dequeue().unwrap().priority, Priority::Critical);
    assert_eq!(queue.dequeue().unwrap().priority, Priority::High);
    assert_eq!(queue.dequeue().unwrap().priority, Priority::Normal);
    assert_eq!(queue.dequeue().unwrap().priority, Priority::Low);
}

#[test]
fn test_fifo_within_same_priority() {
    let queue = PriorityQueue::new();
    
    for i in 0..10 {
        queue.enqueue(i, Priority::Normal);
    }
    
    for i in 0..10 {
        let item = queue.dequeue().unwrap();
        assert_eq!(item.item, i);
        assert_eq!(item.priority, Priority::Normal);
    }
}

#[test]
fn test_high_volume_operations() {
    let queue = PriorityQueue::new();
    
    for i in 0..10000 {
        let priority = match i % 4 {
            0 => Priority::Low,
            1 => Priority::Normal,
            2 => Priority::High,
            _ => Priority::Critical,
        };
        queue.enqueue(i, priority);
    }
    
    assert_eq!(queue.len(), 10000);
    
    let mut prev_priority = Priority::Critical;
    for _ in 0..10000 {
        let item = queue.dequeue().unwrap();
        assert!(item.priority <= prev_priority);
        prev_priority = item.priority;
    }
    
    assert!(queue.is_empty());
}

#[test]
fn test_concurrent_producers() {
    use std::thread;
    
    let queue = PriorityQueue::new();
    let mut handles = vec![];
    
    for i in 0..20 {
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
    
    assert_eq!(queue.len(), 2000);
}

#[test]
fn test_concurrent_consumers() {
    use std::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    let queue = PriorityQueue::new();
    
    for i in 0..2000 {
        queue.enqueue(i, Priority::Normal);
    }
    
    let consumed = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    
    for _ in 0..20 {
        let queue_clone = queue.clone();
        let consumed_clone = consumed.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if queue_clone.dequeue().is_some() {
                    consumed_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(consumed.load(Ordering::SeqCst), 2000);
    assert!(queue.is_empty());
}

#[test]
fn test_mixed_concurrent_operations() {
    use std::thread;
    use std::time::Duration;
    
    let queue = PriorityQueue::new();
    let mut handles = vec![];
    
    for i in 0..10 {
        let queue_clone = queue.clone();
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let priority = if j % 2 == 0 { Priority::High } else { Priority::Normal };
                queue_clone.enqueue(i * 100 + j, priority);
                thread::sleep(Duration::from_micros(1));
            }
        });
        handles.push(handle);
    }
    
    for _ in 0..5 {
        let queue_clone = queue.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                queue_clone.dequeue();
                thread::sleep(Duration::from_micros(1));
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_priority_distribution() {
    let queue = PriorityQueue::new();
    
    for i in 0..100 {
        match i % 4 {
            0 => queue.enqueue(i, Priority::Low),
            1 => queue.enqueue(i, Priority::Normal),
            2 => queue.enqueue(i, Priority::High),
            _ => queue.enqueue(i, Priority::Critical),
        }
    }
    
    assert_eq!(queue.count_by_priority(Priority::Low), 25);
    assert_eq!(queue.count_by_priority(Priority::Normal), 25);
    assert_eq!(queue.count_by_priority(Priority::High), 25);
    assert_eq!(queue.count_by_priority(Priority::Critical), 25);
}

#[test]
fn test_drain_operation() {
    let queue = PriorityQueue::new();
    
    queue.enqueue(1, Priority::Low);
    queue.enqueue(2, Priority::High);
    queue.enqueue(3, Priority::Normal);
    queue.enqueue(4, Priority::Critical);
    
    let items = queue.drain();
    
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].priority, Priority::Critical);
    assert_eq!(items[1].priority, Priority::High);
    assert_eq!(items[2].priority, Priority::Normal);
    assert_eq!(items[3].priority, Priority::Low);
    assert!(queue.is_empty());
}

#[test]
fn test_peek_operation() {
    let queue = PriorityQueue::new();
    
    queue.enqueue(1, Priority::Low);
    queue.enqueue(2, Priority::High);
    
    assert_eq!(queue.peek(), Some(Priority::High));
    assert_eq!(queue.len(), 2);
    
    queue.dequeue();
    assert_eq!(queue.peek(), Some(Priority::Low));
}

#[test]
fn test_clear_operation() {
    let queue = PriorityQueue::new();
    
    for i in 0..100 {
        queue.enqueue(i, Priority::Normal);
    }
    
    assert_eq!(queue.len(), 100);
    queue.clear();
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_with_capacity() {
    let queue: PriorityQueue<i32> = PriorityQueue::with_capacity(1000);
    assert!(queue.capacity() >= 1000);
    
    for i in 0..500 {
        queue.enqueue(i, Priority::Normal);
    }
    
    assert_eq!(queue.len(), 500);
}

#[test]
fn test_try_dequeue() {
    let queue = PriorityQueue::new();
    
    queue.enqueue(42, Priority::Normal);
    
    let item = queue.try_dequeue();
    assert!(item.is_some());
    assert_eq!(item.unwrap().item, 42);
    
    let empty = queue.try_dequeue();
    assert!(empty.is_none());
}

#[test]
fn test_stress_test() {
    use std::thread;
    
    let queue = PriorityQueue::new();
    let mut handles = vec![];
    
    for i in 0..50 {
        let queue_clone = queue.clone();
        let handle = thread::spawn(move || {
            for j in 0..200 {
                let priority = match (i + j) % 4 {
                    0 => Priority::Low,
                    1 => Priority::Normal,
                    2 => Priority::High,
                    _ => Priority::Critical,
                };
                queue_clone.enqueue(i * 200 + j, priority);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(queue.len(), 10000);
}

#[test]
fn test_priority_enum_conversions() {
    assert_eq!(Priority::from_u8(1), Priority::Low);
    assert_eq!(Priority::from_u8(2), Priority::Normal);
    assert_eq!(Priority::from_u8(3), Priority::High);
    assert_eq!(Priority::from_u8(4), Priority::Critical);
    
    assert_eq!(Priority::Low.as_u8(), 1);
    assert_eq!(Priority::Normal.as_u8(), 2);
    assert_eq!(Priority::High.as_u8(), 3);
    assert_eq!(Priority::Critical.as_u8(), 4);
}
