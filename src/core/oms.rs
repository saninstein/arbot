use std::sync::Arc;
use crossbeam_queue::ArrayQueue;
use crate::core::api::OrderListener;
use crate::core::dto::{Order, DTO};


struct OMS {
    queue: Arc<ArrayQueue<DTO>>,
    execution_queue: Arc<ArrayQueue<Order>>,
}

impl OrderListener for OMS {
    fn on_order(&mut self, order: &Order) {

    }
}
