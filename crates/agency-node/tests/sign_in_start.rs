//! A node signs people in from outside in one way, `access` or `oidc`, and refuses to start with both, or with
//! a setting that is not well formed, naming the field.
use agency_adapters::files::DataDir;
use agency_core::config::{AccessConfig, Account, NodeConfig};
use agency_core::oidc_config::OidcConfig;
use agency_core::ports::ConfigStore;
use agency_node::Overrides;
use agency_proto::Id;

/// A change to an Access setting that makes it wrong.
type Edit = Box<dyn FnOnce(&mut AccessConfig)>;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn oidc() -> OidcConfig {
    OidcConfig::new(
        "agency.example.com",
        "https://login.example.com/realms/acme",
        "agency-node",
        "00000000-0000-0000-0000-00000000a11c",
    )
}

fn access() -> AccessConfig {
    AccessConfig {
        public_host: "agency.example.com".into(),
        team_domain: "acme.cloudflareaccess.com".into(),
        audience: "a".repeat(64),
        owner_email: "alice@example.com".into(),
        app_id: None,
        allow_scheme_sign_in: false,
    }
}

/// Opens a node with the configuration `edit` makes, and says why it did not start, if it did not.
fn open_with(edit: impl FnOnce(&mut NodeConfig)) -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let mut config = NodeConfig::new("http://127.0.0.1:1", "127.0.0.1:1", account, "scrum");
    edit(&mut config);
    data.save_config(&config).unwrap();
    agency_node::open(data, Overrides::default())
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tokio::test]
async fn a_node_with_one_well_formed_sign_in_or_none_starts() {
    assert_eq!(open_with(|_| {}), Ok(()), "none");
    assert_eq!(
        open_with(|config| config.access = Some(access())),
        Ok(()),
        "access"
    );
    assert_eq!(
        open_with(|config| config.oidc = Some(oidc())),
        Ok(()),
        "oidc"
    );
    let full = oidc()
        .with_keys_url("https://login.example.com:8443/realms/acme/keys")
        .with_token_header("x-forwarded-access-token")
        .with_email_claim("upn");
    assert_eq!(
        open_with(|config| config.oidc = Some(full)),
        Ok(()),
        "oidc with every optional field"
    );
}

#[tokio::test]
async fn both_settings_stop_the_node() {
    let error = open_with(|config| {
        config.access = Some(access());
        config.oidc = Some(oidc());
    })
    .unwrap_err();
    assert_eq!(
        error,
        "access and oidc are both set: a node signs people in from outside through one of them"
    );
}

#[tokio::test]
async fn an_oidc_setting_that_is_not_well_formed_stops_the_node_and_names_the_field() {
    let cases: Vec<(&str, OidcConfig, &str)> = vec![
        (
            "a public host in capitals",
            OidcConfig::new(
                "Agency.example.com",
                "https://login.example.com",
                "a",
                "s-1",
            ),
            "oidc.publicHost is not a host name",
        ),
        (
            "an issuer on http",
            OidcConfig::new("agency.example.com", "http://login.example.com", "a", "s-1"),
            "oidc.issuer is not https://",
        ),
        (
            "an issuer with a query",
            OidcConfig::new(
                "agency.example.com",
                "https://login.example.com/?x=1",
                "a",
                "s-1",
            ),
            "oidc.issuer is not https://",
        ),
        (
            "an issuer with user information",
            OidcConfig::new(
                "agency.example.com",
                "https://bob@login.example.com",
                "a",
                "s-1",
            ),
            "oidc.issuer is not https://",
        ),
        (
            "an empty audience",
            OidcConfig::new("agency.example.com", "https://login.example.com", "", "s-1"),
            "oidc.audience is empty",
        ),
        (
            "an audience with a space",
            OidcConfig::new(
                "agency.example.com",
                "https://login.example.com",
                "agency node",
                "s-1",
            ),
            "oidc.audience is empty or holds a space",
        ),
        (
            "an owner subject with a space",
            OidcConfig::new(
                "agency.example.com",
                "https://login.example.com",
                "a",
                "alice smith",
            ),
            "oidc.ownerSubject is empty or holds a space",
        ),
        (
            "an empty owner subject",
            OidcConfig::new("agency.example.com", "https://login.example.com", "a", ""),
            "oidc.ownerSubject is empty",
        ),
        (
            "a key set on http",
            oidc().with_keys_url("http://login.example.com/keys"),
            "oidc.keysUrl is not https://",
        ),
        (
            "a token header that is not a header name",
            oidc().with_token_header("x forwarded"),
            "oidc.tokenHeader is not a header name",
        ),
        (
            "an empty email claim",
            oidc().with_email_claim(""),
            "oidc.emailClaim is empty",
        ),
    ];
    for (what, setting, wanted) in cases {
        let error = open_with(|config| config.oidc = Some(setting)).unwrap_err();
        assert!(error.starts_with(wanted), "{what}: {error}");
    }
}

#[tokio::test]
async fn an_access_setting_that_is_not_well_formed_stops_the_node_and_names_the_field() {
    let cases: Vec<(&str, Edit, &str)> = vec![
        (
            "a public host that is not a host name",
            Box::new(|a| a.public_host = "agency".into()),
            "access.publicHost is not a host name",
        ),
        (
            "a team domain that is not Cloudflare's",
            Box::new(|a| a.team_domain = "acme.example.com".into()),
            "access.teamDomain is not <team name>.cloudflareaccess.com",
        ),
        (
            "an audience of the wrong length",
            Box::new(|a| a.audience = "abc".into()),
            "access.audience is not 64 lowercase hex characters",
        ),
        (
            "an audience in capitals",
            Box::new(|a| a.audience = "A".repeat(64)),
            "access.audience is not 64 lowercase hex characters",
        ),
        (
            "an owner that is not an email",
            Box::new(|a| a.owner_email = "alice".into()),
            "access.ownerEmail is not an email address",
        ),
    ];
    for (what, edit, wanted) in cases {
        let mut setting = access();
        edit(&mut setting);
        let error = open_with(|config| config.access = Some(setting)).unwrap_err();
        assert_eq!(error, wanted, "{what}");
    }
}

#[tokio::test]
async fn a_malformed_app_id_does_not_stop_the_node() {
    let mut setting = access();
    setting.app_id = Some("not an app id".into());
    assert_eq!(
        open_with(|config| config.access = Some(setting)),
        Ok(()),
        "only the app's sign-in is off"
    );
}
