use clap::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Protocol {
    Telnet,
    Rlogin,
}

#[derive(Parser, Debug)]
#[command(
    name = "acktel",
    about = "Telnet/Rlogin client",
    version,
    arg_required_else_help = false
)]
pub struct Args {
    /// Hostname to connect to
    pub hostname: Option<String>,

    /// Port number
    pub port: Option<u16>,

    /// Protocol: telnet or rlogin
    #[arg(short = 'P', long, value_enum, default_value_t = Protocol::Telnet)]
    pub protocol: Protocol,

    /// Connection timeout in seconds
    #[arg(short = 't', long, default_value = "30")]
    pub timeout: u32,

    /// Username for authentication
    #[arg(short = 'u', long)]
    pub username: Option<String>,

    /// Password (INSECURE, not recommended)
    #[arg(short = 'p', long)]
    pub password: Option<String>,

    /// Local username for rlogin
    #[arg(short = 'l', long)]
    pub local_user: Option<String>,

    /// Additional username prompt pattern (repeatable)
    #[arg(long)]
    pub user_prompt: Vec<String>,

    /// Additional password prompt pattern (repeatable)
    #[arg(long)]
    pub passwd_prompt: Vec<String>,

    /// Terminal type
    #[arg(short = 'T', long, default_value = "xterm-256color")]
    pub terminal: String,

    /// Display mode: ignore, hex, placeholder, raw
    #[arg(short = 'd', long, value_enum, default_value_t = DisplayModeArg::Raw)]
    pub display: DisplayModeArg,

    /// Configuration file path
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// Log level: error, warn, info, debug
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Show help
    #[arg(short = 'h', long)]
    pub help: bool,

    /// Show version
    #[arg(short = 'v', long)]
    pub version: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DisplayModeArg {
    Ignore,
    Hex,
    Placeholder,
    Raw,
}

impl Args {
    pub fn get_port(&self) -> u16 {
        self.port.unwrap_or_else(|| {
            if self.protocol == Protocol::Rlogin { 513 } else { 23 }
        })
    }

    pub fn get_default_port(protocol: Protocol) -> u16 {
        match protocol {
            Protocol::Telnet => 23,
            Protocol::Rlogin => 513,
        }
    }
}

pub fn print_usage() {
    println!(
        "Usage: acktel [options] <hostname> [port]

Options:
  -P, --protocol <proto>      Protocol: telnet|rlogin (default: telnet)
  -t, --timeout <seconds>     Connection timeout (default: 30)
  -u, --username <username>   Username for authentication
  -p, --password <password>   Password (INSECURE, not recommended)
  -l, --local-user <user>     Local username for rlogin (default: current user)
      --user-prompt <pattern>  Additional username prompt pattern (repeatable)
      --passwd-prompt <pat>    Additional password prompt pattern (repeatable)
  -T, --terminal <type>       Terminal type (default: xterm-256color)
  -d, --display <mode>        Display mode: ignore|hex|placeholder|raw
  -c, --config <file>         Configuration file path
      --log-level <level>     Log level: error|warn|info|debug
  -h, --help                  Show this help message
  -v, --version               Show version information

Runtime commands (prefix with ~):
  ~.   Disconnect
  ~d   Toggle display mode
  ~s   Show connection status
  ~?   Show this help

Examples:
  acktel example.com
  acktel example.com 2323
  acktel --protocol rlogin -l myuser example.com
  acktel -t 60 -T vt100 example.com"
    );
}
