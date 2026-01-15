use std::sync::{Arc, Mutex, Condvar};
use std::sync::mpsc::{self, Sender, Receiver, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

// Error type similar to lwIP's err_t
#[derive(Debug)]
pub enum Err {
    Ok,
    Mem,
    Timeout,
}

// TCP/IP message types
#[derive(Debug)]
pub enum TcpIpMsg {
    Api(Box<dyn FnOnce() + Send>),
    ApiCall {
        function: Box<dyn FnOnce() -> Err + Send>,
        result: Arc<Mutex<Option<Err>>>,
        cvar: Arc<Condvar>,
    },
    Callback(Box<dyn FnOnce() + Send>),
    CallbackStatic(Box<dyn Fn()>),
    // Packet input simulation
    InPkt {
        input_fn: Box<dyn FnOnce() -> Err + Send>,
    },
}

// The main TCP/IP thread struct
pub struct TcpIpThread {
    sender: Sender<TcpIpMsg>,
}

impl TcpIpThread {
    pub fn new() -> Self {
        let (tx, rx): (Sender<TcpIpMsg>, Receiver<TcpIpMsg>) = mpsc::channel();

        thread::spawn(move || {
            tcpip_thread(rx);
        });

        TcpIpThread { sender: tx }
    }

    pub fn callback(&self, func: impl FnOnce() + Send + 'static) -> Err {
        let msg = TcpIpMsg::Callback(Box::new(func));
        self.sender.send(msg).map_err(|_| Err::Mem)?;
        Err::Ok
    }

    pub fn try_callback(&self, func: impl FnOnce() + Send + 'static) -> Err {
        let msg = TcpIpMsg::Callback(Box::new(func));
        match self.sender.try_send(msg) {
            Ok(_) => Err::Ok,
            Err(TrySendError::Full(_)) => Err::Mem,
            Err(TrySendError::Disconnected(_)) => Err::Mem,
        }
    }

    pub fn api_call(&self, func: impl FnOnce() -> Err + Send + 'static) -> Err {
        let result = Arc::new(Mutex::new(None));
        let cvar = Arc::new(Condvar::new());
        let msg = TcpIpMsg::ApiCall {
            function: Box::new(func),
            result: result.clone(),
            cvar: cvar.clone(),
        };

        self.sender.send(msg).map_err(|_| Err::Mem)?;

        // Wait for the TCPIP thread to process the message
        let mut res = result.lock().unwrap();
        while res.is_none() {
            res = cvar.wait(res).unwrap();
        }

        res.take().unwrap_or(Err::Mem)
    }
}

// Main TCP/IP thread loop
fn tcpip_thread(rx: Receiver<TcpIpMsg>) {
    loop {
        match rx.recv() {
            Ok(msg) => handle_msg(msg),
            Err(_) => break, // Channel closed
        }
    }
}

// Handle a single message
fn handle_msg(msg: TcpIpMsg) {
    match msg {
        TcpIpMsg::Api(func) => {
            func();
        }
        TcpIpMsg::ApiCall { function, result, cvar } => {
            let err = function();
            let mut res = result.lock().unwrap();
            *res = Some(err);
            cvar.notify_one();
        }
        TcpIpMsg::Callback(func) => {
            func();
        }
        TcpIpMsg::CallbackStatic(func) => {
            func();
        }
        TcpIpMsg::InPkt { input_fn } => {
            let _ = input_fn(); // Drop result for simplicity
        }
    }
}

// Example usage
fn main() {
    let tcpip = TcpIpThread::new();

    tcpip.callback(|| {
        println!("Callback called in TCP/IP thread!");
    });

    let res = tcpip.api_call(|| {
        println!("API call executed");
        Err::Ok
    });

    println!("API call result: {:?}", res);
}
