use std::iter::Iterator;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use futures::{Future, IntoFuture, Stream};
use hyper;
use hyper::{Request, Response, StatusCode};
use hyper::header::ContentType;
use hyper::server::Service;
use squidtun::{check_proof, generate_session_id};

use session::{NonBlocking, Session};

pub struct TunnelService {
    sessions: Arc<RwLock<Vec<Session>>>,
    password: String,
    remote_host: SocketAddr,
    allowed_diff: u64,
    max_read_size: usize
}

impl TunnelService {
    pub fn new(
        sessions: Arc<RwLock<Vec<Session>>>,
        password: String,
        remote: SocketAddr
    ) -> TunnelService {
        TunnelService{
            sessions: sessions,
            password: password,
            remote_host: remote,
            allowed_diff: 60,
            max_read_size: 65536
        }
    }

    fn connect(&self, proof: &str) -> Box<Future<Item = Vec<u8>, Error = String>> {
        if !check_proof(&self.password, proof, self.allowed_diff) {
            Box::new(Err("incorrect password".to_owned()).into_future())
        } else {
            let sessions = self.sessions.clone();
            let id = generate_session_id();
            Box::new(Session::connect(id.clone(), &self.remote_host)
                .map(move |session| {
                    let sessions: &mut Vec<Session> = &mut sessions.write().unwrap();
                    sessions.push(session);
                    id.as_bytes().to_vec()
                })
                .map_err(|e| format!("connect error: {}", e)))
        }
    }

    fn upload(&self, req: Request, id: String) -> Box<Future<Item = Vec<u8>, Error = String>> {
        let sessions = self.sessions.clone();
        Box::new(req.body().concat2()
            .map_err(|e| format!("read error: {}", e))
            .and_then(move |data| {
                TunnelService::with_session(&sessions, &id, |sess| {
                    sess.write_chunk(&data.to_vec())
                })
            })
            .and_then(|res| {
                match res {
                    NonBlocking::Success(size) => Ok(format!("{}", size)),
                    NonBlocking::Err(err) => Err(format!("write error: {}", err)),
                    NonBlocking::WouldBlock => Err("blocked".to_owned()),
                }.into_future()
            })
            .map(|msg| msg.as_bytes().to_vec()))
    }

    fn download(&self, id: &str) -> Box<Future<Item = Vec<u8>, Error = String>> {
        Box::new(TunnelService::with_session(&self.sessions, id, |sess| {
            sess.read_chunk(self.max_read_size)
        }).and_then(|res| {
            match res {
                NonBlocking::Success(data) => Ok(vec![1].into_iter().chain(data).collect()),
                NonBlocking::Err(err) => Err(format!("io error: {}", err)),
                NonBlocking::WouldBlock => Ok(vec![0]),
            }.into_future()
        }))
    }

    fn close(&self, id: &str) -> Box<Future<Item = Vec<u8>, Error = String>> {
        Box::new(TunnelService::with_session(&self.sessions, id, |sess| {
            sess.send_eof();
            "closed stdout".as_bytes().to_vec()
        }))
    }

    fn invalid(&self) -> Box<Future<Item = Vec<u8>, Error = String>> {
        Box::new(Err("invalid request".to_owned()).into_future())
    }

    fn with_session<R: 'static, F>(
        sessions: &RwLock<Vec<Session>>,
        id: &str,
        f: F
    ) -> Box<Future<Item = R, Error = String>> where F: FnOnce(&mut Session) -> R {
        let sessions: &mut Vec<Session> = &mut sessions.write().unwrap();
        for i in 0..sessions.len() {
            if sessions[i].id == id {
                let result = Box::new(Ok(f(&mut sessions[i])).into_future());
                if sessions[i].is_done() {
                    sessions.remove(i);
                }
                return result;
            }
        }
        Box::new(Err("no session".to_owned()).into_future())
    }
}

impl Service for TunnelService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        let info = RequestInfo::from_request(&req);
        let result = match info {
            RequestInfo::Connect(proof) => self.connect(&proof),
            RequestInfo::Upload(sess_id) => self.upload(req, sess_id),
            RequestInfo::Download(sess_id) => self.download(&sess_id),
            RequestInfo::Close(sess_id) => self.close(&sess_id),
            RequestInfo::Invalid => self.invalid()
        };
        Box::new(result.map(|data| {
            Response::new()
                .with_status(StatusCode::Ok)
                .with_header(ContentType("application/octet-stream".parse().unwrap()))
                .with_body(data)
        }).or_else(|err| {
            let msg = err.as_bytes().to_vec();
            Ok(Response::new()
                .with_status(StatusCode::BadRequest)
                .with_header(ContentType("text/plain".parse().unwrap()))
                .with_body(msg)).into_future()
        }))
    }
}

enum RequestInfo {
    Connect(String),
    Upload(String),
    Download(String),
    Close(String),
    Invalid
}

impl RequestInfo {
    pub fn from_request<B>(req: &Request<B>) -> RequestInfo {
        // Requests are of the form "/<api>/<argument>/unused_data_for_caching".
        let components = req.path().split('/').collect::<Vec<&str>>();
        if components.len() < 3 || components[0].len() != 0 {
            return RequestInfo::Invalid;
        };
        let prefixes: Vec<(&str, Box<Fn(String) -> RequestInfo>)> = vec![
            ("connect", Box::new(|s| RequestInfo::Connect(s))),
            ("upload", Box::new(|s| RequestInfo::Upload(s))),
            ("download", Box::new(|s| RequestInfo::Download(s))),
            ("close", Box::new(|s| RequestInfo::Close(s)))
        ];
        for (prefix, f) in prefixes {
            if components[1] == prefix {
                return f(components[2].to_owned());
            }
        }
        RequestInfo::Invalid
    }
}
