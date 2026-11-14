use clap::Args;

#[derive(Args, Debug, Default)]
pub(crate) struct InteractionArgs {
    #[arg(
        long,
        global = true,
        help = "Not available: this build always works in the background, without the mouse, the keyboard or taking the focus"
    )]
    pub headed: bool,
}
