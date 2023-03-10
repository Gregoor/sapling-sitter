use std::{clone::Clone, sync::Arc};

use tokio::sync::{Notify, RwLock};

pub trait Eventuallable: Clone + Default {}
impl<T> Eventuallable for T where T: Clone + Default {}

#[derive(Debug, Default)]
struct Inner<T: Eventuallable> {
    option_value: Option<T>,
    notify: Arc<Notify>,
}

#[derive(Clone, Debug, Default)]
pub struct Eventually<T: Eventuallable>(Arc<RwLock<Inner<T>>>);

impl<T: Eventuallable> Eventually<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn read(self: &Self) -> T {
        let inner = self.0.read().await;
        loop {
            if let Some(value) = inner.option_value.clone() {
                return value;
            }
            inner.notify.notified().await;
        }
    }

    pub async fn write(self: &Self, value: T) {
        let mut inner = self.0.write().await;
        inner.option_value = Some(value);
        inner.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use tokio::task;

    use super::*;

    #[tokio::test]
    #[ntest::timeout(50)]
    async fn read_and_write() {
        let v = Eventually::new();
        let v2 = v.clone();
        task::spawn(async move {
            assert_eq!(v2.read().await, 42);
        });

        task::spawn(async move {
            v.write(42).await;
        });
    }

    #[tokio::test]
    #[ntest::timeout(50)]
    #[should_panic]
    async fn read_without_write() {
        let v: Eventually<usize> = Eventually::new();
        task::spawn(async move {
            assert_eq!(v.read().await, 42);
        })
        .await
        .expect("should not even get here");
    }
}
