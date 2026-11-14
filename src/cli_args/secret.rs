use clap::{Args, Subcommand};

/// `secret`: passwords a person stores once and scripts and agents use by name.
#[derive(Args, Debug)]
pub(crate) struct SecretArgs {
    #[command(subcommand)]
    pub action: SecretAction,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SecretAction {
    #[command(
        about = "Save a secret under a name; it is typed at a hidden prompt (a person at the terminal), never passed as an argument"
    )]
    Set(SecretSetArgs),
    #[command(
        about = "List the saved secrets' names and what each may be written into (never their values)"
    )]
    List,
    #[command(about = "Delete a saved secret")]
    Delete(SecretNameArgs),
    #[command(
        about = "Write a saved secret into a field, by name, after checking the field, app and site it was saved for"
    )]
    Fill(SecretFillArgs),
}

#[derive(Args, Debug)]
pub(crate) struct SecretSetArgs {
    #[arg(help = "The secret's name: letters, digits, '-', '_' or '.' (e.g. bank-login)")]
    pub name: String,
    #[arg(long, help = "Write it only into this app's windows (e.g. chrome.exe)")]
    pub app: Option<String>,
    #[arg(
        long,
        help = "Write it only on this site or its subdomains (e.g. login.example.com)"
    )]
    pub site: Option<String>,
    #[arg(long, help = "Also allow fields that are not password fields")]
    pub any_field: bool,
    #[arg(
        long,
        help = "Read the value from the first line of standard input instead of a prompt"
    )]
    pub stdin: bool,
}

#[derive(Args, Debug)]
pub(crate) struct SecretNameArgs {
    #[arg(help = "The secret's name")]
    pub name: String,
}

#[derive(Args, Debug)]
pub(crate) struct SecretFillArgs {
    #[arg(
        value_name = "REF",
        help = "The field to write into (@<snapshot_id>:eN)"
    )]
    pub ref_id: String,
    #[arg(help = "The secret's name")]
    pub name: String,
    #[arg(
        long = "timeout-ms",
        help = "Maximum wait for the field, in milliseconds"
    )]
    pub timeout_ms: Option<u64>,
}
