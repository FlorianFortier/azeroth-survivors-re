//! The lobby connection: one WebSocket carrying binary frames, polled once a frame. On the
//! desktop it is a blocking `tungstenite` socket on its own thread, talking to the game over two
//! channels; in the browser it is the page's `WebSocket`, its callbacks filling a queue the game
//! drains. Either way the game sees [`Socket::send`] and [`Socket::poll`], and never blocks.

/// What happened on the socket since the last poll.
#[derive(Debug)]
pub(crate) enum SocketEvent {
    Open,
    Frame(Vec<u8>),
    /// The connection is over, with why.
    Closed(String),
}

pub(crate) use imp::Socket;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::ErrorKind;
    use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
    use std::sync::Mutex;
    use std::time::Duration;

    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;

    use super::SocketEvent;

    pub(crate) struct Socket {
        out: Sender<Vec<u8>>,
        // A `Receiver` is `Send` but not `Sync`; the resource holding the socket must be both.
        events: Mutex<Receiver<SocketEvent>>,
    }

    impl Socket {
        pub(crate) fn connect(url: &str) -> Self {
            let (out, out_rx) = channel::<Vec<u8>>();
            let (ev_tx, events) = channel::<SocketEvent>();
            let url = url.to_string();
            let spawned = std::thread::Builder::new()
                .name("survivors-lobby".into())
                .spawn(move || run(&url, &out_rx, &ev_tx));
            if let Err(e) = spawned {
                let (tx, rx) = channel();
                let _ = tx.send(SocketEvent::Closed(format!(
                    "no thread for the socket: {e}"
                )));
                return Self {
                    out,
                    events: Mutex::new(rx),
                };
            }
            Self {
                out,
                events: Mutex::new(events),
            }
        }

        pub(crate) fn send(&self, frame: Vec<u8>) {
            let _ = self.out.send(frame);
        }

        pub(crate) fn poll(&self) -> Vec<SocketEvent> {
            let Ok(rx) = self.events.lock() else {
                return Vec::new();
            };
            rx.try_iter().collect()
        }
    }

    fn run(url: &str, out: &Receiver<Vec<u8>>, events: &Sender<SocketEvent>) {
        let mut ws = match tungstenite::connect(url) {
            Ok((ws, _)) => ws,
            Err(e) => {
                let _ = events.send(SocketEvent::Closed(format!("cannot reach {url}: {e}")));
                return;
            }
        };
        // A short read timeout turns the blocking read into a poll, so outbound frames go out
        // between reads; tungstenite keeps a partly read frame across the timeouts.
        if let MaybeTlsStream::Plain(tcp) = ws.get_mut() {
            let _ = tcp.set_nodelay(true);
            let _ = tcp.set_read_timeout(Some(Duration::from_millis(4)));
        }
        let _ = events.send(SocketEvent::Open);
        loop {
            loop {
                match out.try_recv() {
                    Ok(frame) => {
                        if let Err(e) = ws.send(Message::binary(frame)) {
                            let _ = events.send(SocketEvent::Closed(e.to_string()));
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    // The game dropped the socket: say goodbye and stop.
                    Err(TryRecvError::Disconnected) => {
                        let _ = ws.close(None);
                        let _ = ws.flush();
                        return;
                    }
                }
            }
            match ws.read() {
                Ok(Message::Binary(b)) => {
                    if events.send(SocketEvent::Frame(b.to_vec())).is_err() {
                        return;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = events.send(SocketEvent::Closed(
                        "the server closed the connection".into(),
                    ));
                    return;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    // Nothing to read: answer pings and push anything buffered.
                    if let Err(e) = ws.flush() {
                        if !matches!(&e, tungstenite::Error::Io(io) if matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
                        {
                            let _ = events.send(SocketEvent::Closed(e.to_string()));
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = events.send(SocketEvent::Closed(e.to_string()));
                    return;
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use send_wrapper::SendWrapper;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    use super::SocketEvent;

    struct Inner {
        ws: Option<WebSocket>,
        events: Rc<RefCell<VecDeque<SocketEvent>>>,
        _on_open: Option<Closure<dyn FnMut(web_sys::Event)>>,
        _on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
        _on_close: Option<Closure<dyn FnMut(web_sys::CloseEvent)>>,
        _on_error: Option<Closure<dyn FnMut(web_sys::Event)>>,
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            if let Some(ws) = &self.ws {
                ws.set_onopen(None);
                ws.set_onmessage(None);
                ws.set_onclose(None);
                ws.set_onerror(None);
                let _ = ws.close();
            }
        }
    }

    pub(crate) struct Socket(SendWrapper<Inner>);

    impl Socket {
        pub(crate) fn connect(url: &str) -> Self {
            let events = Rc::new(RefCell::new(VecDeque::new()));
            let ws = match WebSocket::new(url) {
                Ok(ws) => ws,
                Err(_) => {
                    events
                        .borrow_mut()
                        .push_back(SocketEvent::Closed(format!("cannot open {url}")));
                    return Self(SendWrapper::new(Inner {
                        ws: None,
                        events,
                        _on_open: None,
                        _on_message: None,
                        _on_close: None,
                        _on_error: None,
                    }));
                }
            };
            ws.set_binary_type(BinaryType::Arraybuffer);

            let q = Rc::clone(&events);
            let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                q.borrow_mut().push_back(SocketEvent::Open);
            });
            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

            let q = Rc::clone(&events);
            let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                let Ok(buffer) = e.data().dyn_into::<js_sys::ArrayBuffer>() else {
                    return;
                };
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                q.borrow_mut().push_back(SocketEvent::Frame(bytes));
            });
            ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            let q = Rc::clone(&events);
            let on_close =
                Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |e: web_sys::CloseEvent| {
                    let why = if e.reason().is_empty() {
                        "the connection closed".to_string()
                    } else {
                        e.reason()
                    };
                    q.borrow_mut().push_back(SocketEvent::Closed(why));
                });
            ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

            let q = Rc::clone(&events);
            let target = url.to_string();
            let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                q.borrow_mut()
                    .push_back(SocketEvent::Closed(format!("cannot reach {target}")));
            });
            ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            Self(SendWrapper::new(Inner {
                ws: Some(ws),
                events,
                _on_open: Some(on_open),
                _on_message: Some(on_message),
                _on_close: Some(on_close),
                _on_error: Some(on_error),
            }))
        }

        pub(crate) fn send(&self, frame: Vec<u8>) {
            if let Some(ws) = &self.0.ws {
                if ws.ready_state() == WebSocket::OPEN {
                    let _ = ws.send_with_u8_array(&frame);
                }
            }
        }

        pub(crate) fn poll(&self) -> Vec<SocketEvent> {
            self.0.events.borrow_mut().drain(..).collect()
        }
    }
}
