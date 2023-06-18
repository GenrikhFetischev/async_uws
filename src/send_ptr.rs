#[derive(Clone, Copy)]
pub struct SendPtr<T> {
    pub(crate) ptr: T,
}
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}
