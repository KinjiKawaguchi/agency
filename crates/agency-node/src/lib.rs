//! Agency's node: wires agency-core to its adapters. One node runs per machine; it serves the teams it
//! hosts, keeps replicas of teams on other nodes, sends signed decisions through its outbox, and serves
//! the web client with its Client API.
pub mod budget;
use agency_adapters::access::{is_valid_config, AccessVerifier, CertSource, HttpCerts};
use agency_adapters::apns::{Apns, ApnsHosts};
use agency_adapters::client_api::{self, Assets, ClientApi};
use agency_adapters::files::DataDir;
use agency_adapters::funding::UsageFiles;
use agency_adapters::github::{CachedDelivery, GithubDelivery};
use agency_adapters::handoff;
#[cfg(target_os = "macos")]
use agency_adapters::keychain_root_keys::KeychainRootKeys;
use agency_adapters::launch::Launch;
use agency_adapters::logging::Logger;
use agency_adapters::memory::NoNotifier;
use agency_adapters::metrics::serve::record_request;
use agency_adapters::metrics::Recorder;
use agency_adapters::peers::HttpPeers;
use agency_adapters::refusal_log::RefusalLog;
use agency_adapters::root_keys::FileRootKeys;
use agency_adapters::runtime::CommandRuntime;
use agency_adapters::server;
use agency_adapters::system::{OsRandom, SystemClock, TokioWriteLock};
use agency_adapters::usage_collector;
use agency_adapters::web_push::{WebPush, WithWebPush};
use agency_core::config::{NodeConfig, RootKeyStore};
use agency_core::logging_config::LogLevel;
use agency_core::ports::ConfigStore as _;
use agency_core::ports::{Clock, DeliverySource, Notifier, Random, RootKeys, TeamRuntime};
use agency_core::{Node, Ports};
use axum::Router;
use std::borrow::Cow;
use std::sync::Arc;

/// How long what GitHub said is kept, so that reading a screen does not spend GitHub's rate limit.
const DELIVERY_KEPT: std::time::Duration = std::time::Duration::from_secs(600);

/// Ports to use instead of the real ones (tests).
#[derive(Default)]
pub struct Overrides {
    pub runtime: Option<Arc<dyn TeamRuntime>>,
    pub clock: Option<Arc<dyn Clock>>,
    pub random: Option<Arc<dyn Random>>,
    pub certs: Option<Arc<dyn CertSource>>,
    pub notifier: Option<Arc<dyn Notifier>>,
    pub delivery: Option<Arc<dyn DeliverySource>>,
    /// The program and data directory a client tells the owner to run (default: this process).
    pub launch: Option<Launch>,
    /// Where the org root keys this node holds are kept (default: files outside the data directory).
    pub root_keys: Option<Arc<dyn RootKeys>>,
}

/// The store of org root keys the configuration names.
fn root_keys(store: RootKeyStore) -> anyhow::Result<Arc<dyn RootKeys>> {
    match store {
        RootKeyStore::Files => Ok(Arc::new(FileRootKeys::at_default_dir())),
        #[cfg(target_os = "macos")]
        RootKeyStore::Keychain => Ok(Arc::new(KeychainRootKeys::login())),
        #[cfg(not(target_os = "macos"))]
        RootKeyStore::Keychain => {
            anyhow::bail!("rootKeys \"keychain\" is only for macOS; use \"files\"")
        }
    }
}

/// APNs when the node has a push setting whose key can be read; otherwise nothing is sent to tokens.
fn token_notifier(
    config: &NodeConfig,
    clock: Arc<dyn Clock>,
    logger: &Logger,
) -> Arc<dyn Notifier> {
    let Some(push) = config.push.clone() else {
        return Arc::new(NoNotifier);
    };
    match Apns::new(push, ApnsHosts::default(), clock) {
        Ok(apns) => Arc::new(apns.with_node_id(agency_proto::client::node_id(&config.url))),
        Err(error) => {
            logger.warn("push", &format!("push notifications are off: {error}"));
            Arc::new(NoNotifier)
        }
    }
}

/// The contact a push service may reach this node's operator at, when the configuration names none: the
/// node's public host name, else its origin.
fn web_push_subject(config: &NodeConfig) -> String {
    match &config.access {
        Some(access) => format!("https://{}", access.public_host),
        None => config.url.clone(),
    }
}

/// What to tell the operator when the contact sent to push services is not one they accept: without
/// `webPush.subject` and without `access`, it is the node's own origin, which on a machine's own address is
/// `http://127.0.0.1:...`, and Apple's push service refuses a contact that is not `https` or `mailto:`.
pub fn web_push_subject_warning(config: &NodeConfig) -> Option<String> {
    let has_own = config
        .web_push
        .as_ref()
        .is_some_and(|web| web.subject.is_some());
    (!has_own && !web_push_subject(config).starts_with("https://")).then(|| {
        "web push has no contact for push services that is https; set webPush.subject to a mailto: address (Apple refuses the node's http address)".to_owned()
    })
}

/// The node's notifier: APNs for tokens, and, with a `webPush` setting, browsers' push services for
/// subscriptions (the VAPID key is made in the data directory on first use).
fn notifier(
    config: &NodeConfig,
    data_dir: &std::path::Path,
    clock: Arc<dyn Clock>,
    logger: &Logger,
) -> Arc<dyn Notifier> {
    let tokens = token_notifier(config, clock.clone(), logger);
    let Some(web_config) = &config.web_push else {
        return tokens;
    };
    let key_path = data_dir.join("web-push-vapid.key");
    if let Err(error) = web_config.check() {
        logger.warn("push", &format!("web push is off: {error}"));
        return tokens;
    }
    if let Some(warning) = web_push_subject_warning(config) {
        logger.warn("push", &warning);
    }
    match WebPush::new(web_config, web_push_subject(config), &key_path, clock) {
        Ok(web) => Arc::new(WithWebPush { tokens, web }),
        Err(error) => {
            logger.warn("push", &format!("web push is off: {error}"));
            tokens
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
    logger: Logger,
    recorder: Recorder,
}

/// Opens the node whose configuration and account key live in `data`.
pub fn open(data: DataDir, overrides: Overrides) -> anyhow::Result<Opened> {
    let key = data.load_key()?;
    let data_dir = data.root().to_owned();
    let launch = overrides.launch.or_else(|| Launch::of_process(&data_dir));
    let clock = overrides.clock.unwrap_or_else(|| Arc::new(SystemClock));
    let random = overrides.random.unwrap_or_else(|| Arc::new(OsRandom));
    let config = data.load_config()?;
    config.logging.check().map_err(anyhow::Error::msg)?;
    config.metrics.check().map_err(anyhow::Error::msg)?;
    let logger = Logger::open(data_dir.join("logs"), config.logging.clone(), clock.clone());
    let recorder = Recorder::new(data_dir.join("metrics"), clock.clone());
    let notifier = match overrides.notifier {
        Some(notifier) => notifier,
        None => notifier(&config, &data_dir, clock.clone(), &logger),
    };
    let ports = Ports {
        store: Arc::new(data),
        peers: Arc::new(HttpPeers::new()?),
        runtime: overrides.runtime.unwrap_or_else(|| {
            Arc::new(CommandRuntime::with_log(
                logger.sink(LogLevel::Warn, "runtime"),
            ))
        }),
        clock: clock.clone(),
        random: random.clone(),
        lock: Arc::new(TokioWriteLock::default()),
        funding: Arc::new(UsageFiles::new(data_dir.clone())),
        delivery: overrides.delivery.unwrap_or_else(|| {
            Arc::new(CachedDelivery::new(
                GithubDelivery::default(),
                DELIVERY_KEPT,
            ))
        }),
        notifier,
        journal: Arc::new(logger.clone()),
        root_keys: match overrides.root_keys {
            Some(root_keys) => root_keys,
            None => root_keys(config.root_keys)?,
        },
    };
    let node = Arc::new(Node::open(ports, key)?);
    Ok(Opened {
        node,
        data_dir,
        clock,
        random,
        certs: overrides.certs,
        launch,
        logger,
        recorder,
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
    /// The node's log.
    pub fn logger(&self) -> &Logger {
        &self.logger
    }

    /// The node's own measurements: the recorder every request is timed into.
    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    /// Starts reading the usage of the funding sources that have a collector, for as long as the
    /// node runs.
    pub fn start_usage_collector(&self) {
        tokio::spawn(usage_collector::run(
            self.node.clone(),
            self.data_dir.clone(),
            self.clock.clone(),
            {
                let sink = self.logger.sink(LogLevel::Warn, "usage");
                Arc::new(move |line: String| sink(&line))
            },
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
            self.logger.warn("access", "the access setting is malformed (publicHost, teamDomain, audience, ownerEmail); requests through Cloudflare are refused");
            return Ok(None);
        }
        if config.app_id.is_some() && handoff::app_id(&config).is_none() {
            self.logger.warn("access", "the access appId is malformed (<team id>.<bundle id>); the app sign-in handoff and the app association file are off");
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
            log: self.logger.sink(LogLevel::Warn, "access"),
            metrics: self.recorder.clone(),
            token,
            port,
            access,
            assets,
            launch: self.launch.clone(),
            client_notes: Default::default(),
            handoff: handoff::Codes::default(),
            claim_attempts: Default::default(),
        };
        let refusals = RefusalLog::new(self.logger.sink(LogLevel::Warn, "refused"));
        let app = server::router_with_log(self.node.clone(), refusals)
            .merge(client_api::router(Arc::new(api)))
            .layer(axum::middleware::from_fn_with_state(
                self.recorder.clone(),
                record_request,
            ))
            // Outermost, so no route, guard or measurement sees a path that could be read as another.
            .layer(axum::middleware::from_fn(
                agency_adapters::raw_path::refuse_ambiguous_paths,
            ));
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
