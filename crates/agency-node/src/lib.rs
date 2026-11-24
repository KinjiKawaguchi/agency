//! Agency's node: wires agency-core to its adapters. One node runs per machine; it serves the teams it
//! hosts, keeps replicas of teams on other nodes, sends signed decisions through its outbox, and serves
//! the web client with its Client API.
use agency_adapters::access::{is_valid_config, AccessVerifier, CertSource, HttpCerts};
use agency_adapters::client_api::{self, Assets, ClientApi};
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::peers::HttpPeers;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::server;
use agency_adapters::system::{OsRandom, SystemClock, TokioWriteLock};
use agency_core::ports::{Clock, Random, TeamRuntime};
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
}

/// The node and the parts its HTTP server needs.
pub struct Opened {
    pub node: Arc<Node>,
    clock: Arc<dyn Clock>,
    random: Arc<dyn Random>,
    certs: Option<Arc<dyn CertSource>>,
}

/// Opens the node whose configuration and account key live in `data`.
pub fn open(data: DataDir, overrides: Overrides) -> anyhow::Result<Opened> {
    let key = data.load_key()?;
    let clock = overrides.clock.unwrap_or_else(|| Arc::new(SystemClock));
    let random = overrides.random.unwrap_or_else(|| Arc::new(OsRandom));
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new()?),
        runtime: overrides
            .runtime
            .unwrap_or_else(|| Arc::new(CommandRuntime::default())),
        clock: clock.clone(),
        random: random.clone(),
        lock: Arc::new(TokioWriteLock::default()),
        funding: Arc::new(UsageFiles::new(clock.clone())),
    };
    let node = Arc::new(Node::open(ports, key)?);
    Ok(Opened {
        node,
        clock,
        random,
        certs: overrides.certs,
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
        let api = ClientApi {
            node: self.node.clone(),
            token,
            port,
            access: self.access()?,
            assets,
        };
        Ok(server::router(self.node.clone()).merge(client_api::router(Arc::new(api))))
    }
}
