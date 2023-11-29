use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use uwebsockets_rs::uws_loop::{loop_defer, UwsLoop};

#[derive(Default)]
struct LoopDeferFutureState {
  waker: Option<Waker>,
  is_completed: bool
}
pub struct LoopDeferFuture {
  state: Arc<Mutex<LoopDeferFutureState>>,
}

impl LoopDeferFuture {
  pub fn new<C>(callback: C, uws_loop: UwsLoop) -> Self
    where C: FnOnce() + Send + 'static
  {
    let state: Arc<Mutex<LoopDeferFutureState>> = Default::default();
    let state_to_move = state.clone();
    let closure = move || {
      callback();
      let mut state = state_to_move.lock().unwrap();
      if let Some(waker) = state.waker.take() {
        state.is_completed = true;
        waker.wake()
      }
    };

    tokio::spawn(async move {
      loop_defer(uws_loop, closure);
    });

    LoopDeferFuture { state }
  }
}

impl Future for LoopDeferFuture {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut state = self.state.lock().unwrap();
    if state.is_completed {
      return Poll::Ready(());
    }
    state.waker = Some(cx.waker().clone());
    Poll::Pending
  }
}
