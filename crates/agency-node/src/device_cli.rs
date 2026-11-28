//! `agency device-add` and the CLI device. The account key adds only a person's first device, and only
//! here, on the node's terminal, after the person compares pairing codes. Later devices are added by
//! one of the person's devices: from a client, or here with the CLI device once it is added.
use agency_adapters::files::DataDir;
use agency_core::devices::{add_device, pending_requests, request_device, Added, DeviceSigner};
use agency_core::Node;
use agency_proto::client::{DeviceChangeOutcome, DeviceRequestInput};
use agency_proto::org::printable;
use agency_proto::{AccountKey, Id, PublicKey};
use anyhow::{bail, Result};
use clap::Args;
use std::io::Write;

/// Adds a device that asked through the web client, after you compare its pairing code. With --cli,
/// adds this terminal's own device, which `decide` signs with. The account key adds only your first
/// device; later ones are added from one of your devices.
#[derive(Args)]
pub struct DeviceAdd {
    /// Adds the CLI device (its key stays in the data directory)
    #[arg(long)]
    cli: bool,
}

/// The CLI device's key and id. Without one, the CLI cannot decide.
pub struct CliDevice {
    pub id: Id,
    pub key: AccountKey,
}

impl CliDevice {
    fn of(key: AccountKey) -> Self {
        Self {
            id: key.public().device_id(),
            key,
        }
    }

    pub fn load(data: &DataDir) -> Result<Self> {
        let Some(key) = data.load_device_key()? else {
            bail!("this node has no CLI device, and only devices sign decisions; run `agency device-add --cli` first");
        };
        Ok(Self::of(key))
    }

    pub fn signer(&self) -> DeviceSigner<'_> {
        DeviceSigner {
            id: &self.id,
            key: &self.key,
        }
    }
}

fn ask(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}

fn describe(label: &str, outcome: DeviceChangeOutcome, message: Option<String>) -> String {
    match outcome {
        DeviceChangeOutcome::Applied => format!("{label} added (this node holds the root key)"),
        DeviceChangeOutcome::Delivered => format!("{label} added by the root's node"),
        DeviceChangeOutcome::Pending => format!(
            "{label} waits for the owner of the root key ({})",
            printable(&message.unwrap_or_default())
        ),
        DeviceChangeOutcome::Queued => format!(
            "{label} waits in the outbox for the root's node ({})",
            printable(&message.unwrap_or_default())
        ),
    }
}

/// Adds a device in every org the account belongs to, and says what happened in each. Returns whether
/// some org needs one of the account's other devices to add it.
async fn add_and_report(
    node: &Node,
    label: &str,
    key: &PublicKey,
    cli: Option<&DeviceSigner<'_>>,
) -> bool {
    let mut needs_device = false;
    for (org, outcome) in add_device(node, label, key, cli).await {
        match outcome {
            Added::Done(result) => {
                println!(
                    "Org {org}: {}",
                    describe(label, result.outcome, result.message)
                );
            }
            Added::NeedsDevice => {
                needs_device = true;
                println!(
                    "Org {org}: you already have a device there; add {label} from one of them"
                );
            }
            Added::Failed(error) => println!(
                "Org {org}: {label} not added: {}",
                printable(&error.to_string())
            ),
        }
    }
    needs_device
}

/// `agency device-add --cli`: makes the CLI device's key if needed and adds it.
async fn add_cli_device(node: &Node, data: &DataDir) -> Result<()> {
    let key = match data.load_device_key()? {
        Some(key) => key,
        None => data.create_device_key()?,
    };
    let public = key.public();
    println!(
        "CLI device {} with pairing code {}",
        public.device_id(),
        public.pairing_code()
    );
    if add_and_report(node, "CLI", &public, None).await {
        let input = DeviceRequestInput {
            label: "CLI".into(),
            public_key: public.clone(),
        };
        request_device(node, input)?;
        println!(
            "Asked to be added: approve the CLI device (pairing code {}) from one of your devices",
            public.pairing_code()
        );
    }
    Ok(())
}

/// `agency device-add`: adds a device that asked through the Client API, after the person checks that
/// it shows the same pairing code. Signed with the account key for a first device, otherwise with the
/// CLI device if it was added.
async fn add_requested_device(node: &Node, data: &DataDir) -> Result<()> {
    let requests = pending_requests(node)?;
    if requests.is_empty() {
        bail!("no device is waiting to be added; on the new device, open the web client and ask to add it, then run this again");
    }
    let cli = data.load_device_key()?.map(CliDevice::of);
    let signer = cli.as_ref().map(CliDevice::signer);
    for request in requests.iter().rev() {
        let question = format!(
            "Add \"{}\" with pairing code {}? Check that the device shows the same code.",
            agency_proto::org::printable(&request.label),
            request.pairing_code
        );
        if ask(&question) {
            add_and_report(node, &request.label, &request.public_key, signer.as_ref()).await;
        }
    }
    Ok(())
}

/// Says which node and data directory this run read, so a person with several can tell whether it is
/// the one the device asked.
fn say_which_node(node: &Node, data: &DataDir) {
    let dir = std::path::absolute(data.root()).unwrap_or_else(|_| data.root().to_owned());
    println!(
        "Node {} (data directory {})",
        node.config().url,
        dir.display()
    );
}

pub async fn device_add(node: &Node, data: &DataDir, args: DeviceAdd) -> Result<()> {
    say_which_node(node, data);
    if args.cli {
        add_cli_device(node, data).await
    } else {
        add_requested_device(node, data).await
    }
}
