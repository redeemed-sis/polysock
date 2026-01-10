use crate::modes::oneliner::OnelinerModeParamsBuilder;
use crate::modes::{
    Command,
    oneliner::{OnelinerMode, OnelinerModeCommand},
};
use crate::sock::{
    SocketFactory, SocketParams, TraceCanonicalDecoratorFactory, TraceInfoDecoratorFactory,
    TraceRawDecoratorFactory,
};
use crate::sockets::{
    tcp_client::TcpClientFactory, tcp_server::TcpServerFactory, terminal::SimpleTerminalFactory,
    udp::SocketFactoryUDP, testgen::TestGenFactory,
};

use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand, ValueEnum};

use std::collections::HashMap;
use std::process;
use std::sync::LazyLock;

#[derive(Copy, Clone, ValueEnum)]
enum ExchangeMode {
    Unidir,
    Bidir,
}

#[derive(clap::Args, Clone)]
struct OnelinerArgs {
    /// Exchange mode
    #[arg(value_enum, short, long, default_value_t = ExchangeMode::Unidir)]
    exchange_mode: ExchangeMode,
    /// Blocking input
    #[arg(short, long, default_value_t = false)]
    blocking: bool,
    /// The first socket to bind
    #[arg(short, long, value_parser = PossibleValuesParser::new(FACTORY_MAP.keys()))]
    from_dev: String,
    /// The second socket to bind
    #[arg(short, long, value_parser = PossibleValuesParser::new(FACTORY_MAP.keys()))]
    to_dev: String,
    /// The first socket parameters (JSON format)
    #[arg(long)]
    from_params: Option<SocketParams>,
    /// The second socket parameters (JSON format)
    #[arg(long)]
    to_params: Option<SocketParams>,
    /// Socket info tracing
    #[arg(long, default_value_t = false)]
    trace_info: bool,
    /// Socket data tracing (in raw format)
    #[arg(long, default_value_t = false)]
    trace_raw: bool,
    /// Socket data tracing (in canonical format)
    #[arg(long, default_value_t = false)]
    trace_canon: bool,
    /// From device tracing off
    #[arg(long, default_value_t = false)]
    trace_from_off: bool,
    /// To device tracing off
    #[arg(long, default_value_t = false)]
    trace_to_off: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Oneliner mode (command line prameters management)
    Oneliner(OnelinerArgs),
    /// Not implemented yet
    Script {},
    /// Not implemented yet
    Repl {},
}

#[derive(Parser)]
#[command(subcommand_negates_reqs = true)]
pub struct PolySockArgs {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Option<Commands>,
}

type FactoryCallback = Box<dyn Fn() -> Box<dyn SocketFactory> + Send + Sync>;
macro_rules! factory_callback_create {
    ($factory: expr) => {
        Box::new(|| Box::new($factory) as Box<dyn SocketFactory>) as FactoryCallback
    };
}

static FACTORY_MAP: LazyLock<HashMap<&'static str, FactoryCallback>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("udp", factory_callback_create!(SocketFactoryUDP::new()));
    m.insert(
        "stdio",
        factory_callback_create!(SimpleTerminalFactory::new()),
    );
    m.insert(
        "tcp-client",
        factory_callback_create!(TcpClientFactory::new()),
    );
    m.insert("tcp-server", factory_callback_create!(TcpServerFactory::new()));
    m.insert("test-gen", factory_callback_create!(TestGenFactory::new()));
    m
});

impl PolySockArgs {
    pub fn get_scenario() -> Box<dyn Command> {
        let args = Self::parse();
        let command = match &args.command.unwrap_or_else(|| {
            eprintln!("Default command line parameters or subcommands are not provided!");
            process::exit(1)
        }) {
            Commands::Oneliner(args) => Self::get_oneliner_command(args),
            Commands::Repl {} => {
                panic!("Repl mode is not implemented yet!");
            }
            Commands::Script {} => {
                panic!("Script mode is not implemented yet!");
            }
        };

        command.unwrap_or_else(|| {
            eprintln!("Command parsing failed!");
            process::exit(1)
        })
    }
    fn get_oneliner_command(args: &OnelinerArgs) -> Option<Box<dyn Command>> {
        let set_decorators =
            |mut f: Box<dyn SocketFactory>, args: &OnelinerArgs| -> Box<dyn SocketFactory> {
                // Socket info must be printed firstly
                if args.trace_info {
                    f = TraceInfoDecoratorFactory::new(f);
                }
                // Raw data should be printed after socket info
                if args.trace_raw {
                    f = TraceRawDecoratorFactory::new(f);
                }
                // Canonical data is the last
                if args.trace_canon {
                    f = TraceCanonicalDecoratorFactory::new(f);
                }
                f
            };
        let mut f_factory = if let Some(cb) = FACTORY_MAP.get(args.from_dev.as_str()) {
            cb()
        } else {
            eprintln!("Socket type {} not found! Exiting...", args.from_dev);
            process::exit(1);
        };
        let mut t_factory = if let Some(cb) = FACTORY_MAP.get(args.to_dev.as_str()) {
            cb()
        } else {
            eprintln!("Socket type {} not found! Exiting...", args.to_dev);
            process::exit(1);
        };

        // Set decorators, if it is not disabled for
        // this direction
        if !args.trace_from_off {
            f_factory = set_decorators(f_factory, args);
        }
        if !args.trace_to_off {
            t_factory = set_decorators(t_factory, args);
        }

        let f_params = args.from_params.clone().unwrap_or_default();
        let to_params = args.to_params.clone().unwrap_or_default();

        let oneliner_params = OnelinerModeParamsBuilder::default()
            .f_params(f_params)
            .to_params(to_params)
            .bidir(matches!(args.exchange_mode, ExchangeMode::Bidir))
            .blocking(args.blocking)
            .build()
            .unwrap_or_else(|e| {
                eprintln!("Oneliner command parameters building failed: {e}");
                process::exit(1)
            });
        Some(Box::new(OnelinerModeCommand::new(OnelinerMode::new(
            f_factory,
            t_factory,
            oneliner_params,
        ))))
    }
}
