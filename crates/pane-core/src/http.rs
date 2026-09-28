//! Guests' web requests: `wasi:http@0.3.0`'s `client.send`, which Pane
//! sends for them.
//!
//! Wasmtime's `wasi:http` implementation turns a guest's request into an
//! [`http::Request`] and hands it to [`Sender::send_request`], which connects
//! (TLS for `https`, trusting the system's certificates), sends it over
//! HTTP/1.1 and hands back the response as it arrives. Everything runs on the
//! runtime thread, inside the guest call that asked for it: when that call
//! stops (its generation ended, or the launcher cancelled a search the user
//! replaced), the instance and its store are dropped, and with them the
//! connection, so an abandoned request goes no further.
//!
//! Code whose generation has ended cannot start a request, like the other
//! host imports it may not use any more. Otherwise extensions are trusted
//! code: any `http` or `https` address is allowed, as their own native code
//! would be.

use std::future::Future;
use std::pin::{Pin, pin};
use std::sync::{Arc, OnceLock};
use std::task::{Poll, ready};
use std::time::Duration;

use http::uri::Scheme;
use http_body_util::BodyExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::{Error, RequestOptions, WasiBody, WasiHttpHooks};

use crate::extension_data::PackageData;

/// How long connecting may take when the guest sets no timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the response's head may take once the request is sent, when the
/// guest sets no timeout.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(30);

/// The rest of a response being received, once its head has arrived: it
/// completes when the connection finishes, with why if it failed.
type Receiving = Box<dyn Future<Output = Result<(), Error>> + Send>;

/// What sending a request resolves to.
type Sent = Box<dyn Future<Output = Result<(http::Response<WasiBody>, Receiving), Error>> + Send>;

/// Sends one guest instance's requests. It knows the instance's extension
/// data only to refuse code whose generation has ended.
pub(crate) struct Sender {
    data: Option<PackageData>,
}

impl Sender {
    pub(crate) fn new(data: Option<PackageData>) -> Sender {
        Sender { data }
    }
}

impl WasiHttpHooks for Sender {
    fn send_request(
        &mut self,
        request: http::Request<WasiBody>,
        options: Option<RequestOptions>,
        // Where Wasmtime reports whether the guest read the response through;
        // nothing here needs it.
        _received: Box<dyn Future<Output = Result<(), Error>> + Send>,
    ) -> Sent {
        if self.data.as_ref().and_then(PackageData::stopped).is_some() {
            // Code whose generation ended starts no more work.
            return Box::new(async { Err(Error::HttpRequestDenied) });
        }
        Box::new(send(request, options))
    }
}

/// A connection, plain or TLS.
trait Connection: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}
impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static> Connection for T {}

/// Connects to the request's host and sends it, resolving once the
/// response's head has arrived; its body follows as it is received.
async fn send(
    mut request: http::Request<WasiBody>,
    options: Option<RequestOptions>,
) -> Result<(http::Response<WasiBody>, Receiving), Error> {
    let uri = request.uri();
    let tls = uri.scheme() == Some(&Scheme::HTTPS);
    let authority = uri.authority().ok_or(Error::HttpRequestUriInvalid)?;
    let host = authority
        .host()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = authority.port_u16().unwrap_or(if tls { 443 } else { 80 });
    let connect_timeout = options
        .and_then(|options| options.connect_timeout)
        .unwrap_or(CONNECT_TIMEOUT);
    let first_byte_timeout = options
        .and_then(|options| options.first_byte_timeout)
        .unwrap_or(FIRST_BYTE_TIMEOUT);

    let stream = match timeout(connect_timeout, TcpStream::connect((host.as_str(), port))).await {
        Ok(stream) => stream.map_err(Error::Connect)?,
        Err(_) => return Err(Error::ConnectionTimeout),
    };
    let stream: Box<dyn Connection> = if tls {
        let config = tls_config().map_err(|problem| Error::InternalError(Some(problem)))?;
        let name = rustls::pki_types::ServerName::try_from(host)
            .map_err(|_| Error::HttpRequestUriInvalid)?;
        let connecting = tokio_rustls::TlsConnector::from(config).connect(name, stream);
        match timeout(connect_timeout, connecting).await {
            Ok(stream) => Box::new(stream.map_err(Error::Tls)?),
            Err(_) => return Err(Error::ConnectionTimeout),
        }
    } else {
        Box::new(stream)
    };
    let (mut sender, connection) = timeout(
        connect_timeout,
        hyper::client::conn::http1::handshake(TokioIo::new(stream)),
    )
    .await
    .map_err(|_| Error::ConnectionTimeout)??;

    // The request line names only the path: the host travels in its Host
    // header, which Wasmtime set.
    let path = request
        .uri()
        .path_and_query()
        .map_or("/", |path| path.as_str())
        .to_owned();
    *request.uri_mut() = http::Uri::builder()
        .path_and_query(path)
        .build()
        .map_err(|_| Error::HttpRequestUriInvalid)?;

    let mut response = pin!(async {
        timeout(first_byte_timeout, sender.send_request(request))
            .await
            .map_err(|_| Error::ConnectionReadTimeout)?
            .map_err(Error::from)
    });
    let mut connection = Some(connection);
    // The connection does the reading and writing: drive it while waiting
    // for the response's head.
    let response = std::future::poll_fn(|cx| match response.as_mut().poll(cx) {
        Poll::Ready(result) => Poll::Ready(result),
        Poll::Pending => {
            let Some(driving) = connection.as_mut() else {
                return Poll::Pending;
            };
            let finished = ready!(Pin::new(driving).poll(cx));
            connection = None;
            match finished {
                Ok(()) => response.as_mut().poll(cx),
                Err(error) => Poll::Ready(Err(Error::from(error))),
            }
        }
    })
    .await?;
    let receiving: Receiving = Box::new(async move {
        match connection {
            Some(connection) => connection.await.map_err(Error::from),
            None => Ok(()),
        }
    });
    Ok((
        response.map(|body| body.map_err(Error::from).boxed_unsync()),
        receiving,
    ))
}

/// TLS settings trusting the system's certificates, read once.
fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    static CONFIG: OnceLock<Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            let found = rustls_native_certs::load_native_certs();
            for certificate in found.certs {
                // A certificate the system lists but rustls cannot use is
                // skipped, as browsers skip them.
                let _ = roots.add(certificate);
            }
            if roots.is_empty() {
                let why = found
                    .errors
                    .first()
                    .map_or_else(|| "none are installed".to_owned(), ToString::to_string);
                return Err(format!(
                    "no certificates of the system could be read ({why})"
                ));
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map(|config| Arc::new(config.with_root_certificates(roots).with_no_client_auth()))
                .map_err(|error| error.to_string())
        })
        .clone()
}
