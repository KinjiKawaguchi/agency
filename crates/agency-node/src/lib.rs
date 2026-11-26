//! Agency's node: wires agency-core to its adapters. One node runs per machine; it serves the teams it
//! hosts, keeps replicas of teams on other nodes, sends signed decisions through its outbox, and serves
//! the web client with its Client API.
use agency_adapters::access::{is_valid_config, AccessVerifier, CertSource, HttpCerts};
use agency_adapters::apns::{Apns, ApnsHosts};
use agency_adapters::client_api::{self, Assets, ClientApi};
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::handoff;
use agency_adapters::launch::Launch;
use agency_adapters::memory::NoNotifier;
use agency_adapters::peers::HttpPeers;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::server;
use agency_adapters::system::{OsRandom, SystemClock, TokioWriteLock};
use agency_adapters::usage_collector;
use agency_core::config::NodeConfig;
use agency_core::ports::ConfigStore as _;
use agency_core::ports::{Clock, Notifier, Random, TeamRuntime};
use agency_core::{Node, Ports};
use axum::Router;
use std::borrow::Cow;
use std::sync::Arc;

/// Ports to use instead of the real ones (tests).
#[derive(Default)]
pub struct Overrides {
    pub runtime: Option<Arc<dyn TeamRuntime>>,
    pub clock: Option<Arc<dyn Clock>>,
    pub random: Option<Arc<dyn Random>>,
    pub certs: Option<Arc<dyn CertSource>>,
    pub notifier: Option<Arc<dyn Notifier>>,
    /// The program and data directory a client tells the owner to run (default: this process).
    pub launch: Option<Launch>,
}

/// APNs when the node has a push setting whose key can be read; otherwise nothing is sent.
fn notifier(config: &NodeConfig, clock: Arc<dyn Clock>) -> Arc<dyn Notifier> {
    let Some(push) = config.push.clone() else {
        return Arc::new(NoNotifier);
    };
    match Apns::new(push, ApnsHosts::default(), clock) {
        Ok(apns) => Arc::new(apns),
        Err(error) => {
            eprintln!("agency: push notifications are off: {error}");
            Arc::new(NoNotifier)
        }
    }
}

/// The node and the parts its HTTP server needs.
pub struct Opened {
    pub node: Arc<Node>,
    data_dir: std::path::PathBuf,
    clock: Arc<dyn Clock>,
    random: Arc<dyn Random>,
    certs: Option<Arc<dyn CertSource>>,
    launch: Option<Launch>,
}

/// Opens the node whose configuration and account key live in `data`.
pub fn open(data: DataDir, overrides: Overrides) -> anyhow::Result<Opened> {
    let key = data.load_key()?;
    let data_dir = data.root().to_owned();
    let launch = overrides.launch.or_else(|| Launch::of_process(&data_dir));
    let clock = overrides.clock.unwrap_or_else(|| Arc::new(SystemClock));
    let random = overrides.random.unwrap_or_else(|| Arc::new(OsRandom));
    let notifier = match overrides.notifier {
        Some(notifier) => notifier,
        None => notifier(&data.load_config()?, clock.clone()),
    };
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new()?),
        runtime: overrides
            .runtime
            .unwrap_or_else(|| Arc::new(CommandRuntime::default())),
        clock: clock.clone(),
        random: random.clone(),
        lock: Arc::new(TokioWriteLock::default()),
        funding: Arc::new(UsageFiles::new(data_dir.clone())),
        notifier,
    };
    let node = Arc::new(Node::open(ports, key)?);
    Ok(Opened {
        node,
        data_dir,
        clock,
        random,
        certs: overrides.certs,
        launch,
    })
}

/// The built web client (`web/dist`), embedded at compile time. Empty when it was not built.
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct WebDist;

struct EmbeddedWeb;

impl Assets for EmbeddedWeb {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        WebDist::get(path).map(|file| file.data)
    }
}

impl Opened {
    /// Starts reading the usage of the funding sources that have a collector, for as long as the
    /// node runs.
    pub fn start_usage_collector(&self) {
        tokio::spawn(usage_collector::run(
            self.node.clone(),
            self.data_dir.clone(),
            self.clock.clone(),
        ));
    }

    /// A fresh write token for the page: 32 random bytes as hex.
    fn new_token(&self) -> String {
        [self.random.bytes16(), self.random.bytes16()]
            .concat()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn access(&self) -> anyhow::Result<Option<Arc<AccessVerifier>>> {
        let Some(config) = self.node.config().access else {
            return Ok(None);
        };
        if !is_valid_config(&config) {
            eprintln!("agency: the access setting is malformed (publicHost, teamDomain, audience, ownerEmail); requests through Cloudflare are refused");
            return Ok(None);
        }
        if config.app_id.is_some() && handoff::app_id(&config).is_none() {
            eprintln!("agency: the access appId is malformed (<team id>.<bundle id>); the app sign-in handoff and the app association file are off");
        }
        let certs = match &self.certs {
            Some(certs) => certs.clone(),
            None => Arc::new(HttpCerts::new()?),
        };
        Ok(Some(Arc::new(AccessVerifier::new(
            config,
            certs,
            self.clock.clone(),
        ))))
    }

    /// The federation routes, the Client API, and the web client, for a node listening on `port`.
    /// Serve it with `into_make_service_with_connect_info::<SocketAddr>()`.
    pub fn app(&self, port: u16) -> anyhow::Result<Router> {
        self.app_with(port, self.new_token(), Arc::new(EmbeddedWeb))
    }

    /// `app` with a given write token and web client files (tests).
    pub fn app_with(
        &self,
        port: u16,
        token: String,
        assets: Arc<dyn Assets>,
    ) -> anyhow::Result<Router> {
        let access = self.access()?;
        let is_public = access.is_some();
        let api = ClientApi {
            node: self.node.clone(),
            token,
            port,
            access,
            assets,
            launch: self.launch.clone(),
        };
        let app = server::router(self.node.clone()).merge(client_api::router(Arc::new(api)));
        Ok(if is_public {
            app.layer(axum::middleware::map_response(strict_transport))
        } else {
            app
        })
    }
}

/// HSTS for a node reached at a public host name over HTTPS: browsers then refuse plain HTTP to it for a
/// year. Without `includeSubDomains`, it says nothing about other names under the same domain.
/// Browsers ignore the header on plain HTTP, so local answers carrying it change nothing.
pub const STRICT_TRANSPORT_SECURITY: &str = "max-age=31536000";

async fn strict_transport(mut response: axum::response::Response) -> axum::response::Response {
    response.headers_mut().insert(
        axum::http::header::STRICT_TRANSPORT_SECURITY,
        axum::http::HeaderValue::from_static(STRICT_TRANSPORT_SECURITY),
    );
    response
}
