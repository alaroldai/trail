use std::sync::Arc;

pub trait With<T: ?Sized> {
  fn with(self, item: &Arc<T>) -> Self;
}
