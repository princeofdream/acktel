/// IAC command bytes (RFC 854)
pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const GA: u8 = 249;
pub const EL: u8 = 248;
pub const EC: u8 = 247;
pub const AYT: u8 = 246;
pub const AO: u8 = 245;
pub const IP: u8 = 244;
pub const BRK: u8 = 243;
pub const DM: u8 = 242;
pub const NOP: u8 = 241;
pub const SE: u8 = 240;

/// Telnet option codes (RFC 855 and various option RFCs)
pub const TELOPT_BINARY: u8 = 0;
pub const TELOPT_ECHO: u8 = 1;
pub const TELOPT_RCP: u8 = 2;
pub const TELOPT_SGA: u8 = 3;
pub const TELOPT_NAMS: u8 = 4;
pub const TELOPT_STATUS: u8 = 5;
pub const TELOPT_TM: u8 = 6;
pub const TELOPT_RCTE: u8 = 7;
pub const TELOPT_NAOL: u8 = 8;
pub const TELOPT_NAOP: u8 = 9;
pub const TELOPT_NAOCRD: u8 = 10;
pub const TELOPT_NAOHTS: u8 = 11;
pub const TELOPT_NAOHTD: u8 = 12;
pub const TELOPT_NAOFFD: u8 = 13;
pub const TELOPT_NAOVTS: u8 = 14;
pub const TELOPT_NAOVTD: u8 = 15;
pub const TELOPT_NAOLFD: u8 = 16;
pub const TELOPT_XASCII: u8 = 17;
pub const TELOPT_LOGOUT: u8 = 18;
pub const TELOPT_BM: u8 = 19;
pub const TELOPT_DET: u8 = 20;
pub const TELOPT_SUPDUP: u8 = 21;
pub const TELOPT_SUPDUPOUTPUT: u8 = 22;
pub const TELOPT_SNDLOC: u8 = 23;
pub const TELOPT_TTYPE: u8 = 24;
pub const TELOPT_EOR: u8 = 25;
pub const TELOPT_TUID: u8 = 26;
pub const TELOPT_OUTMRK: u8 = 27;
pub const TELOPT_TTYLOC: u8 = 28;
pub const TELOPT_3270REGIME: u8 = 29;
pub const TELOPT_X3PAD: u8 = 30;
pub const TELOPT_NAWS: u8 = 31;
pub const TELOPT_TSPEED: u8 = 32;
pub const TELOPT_LFLOW: u8 = 33;
pub const TELOPT_LINEMODE: u8 = 34;
pub const TELOPT_XDISPLOC: u8 = 35;
pub const TELOPT_OLD_ENVIRON: u8 = 36;
pub const TELOPT_AUTHENTICATION: u8 = 37;
pub const TELOPT_ENCRYPT: u8 = 38;
pub const TELOPT_NEW_ENVIRON: u8 = 39;
pub const TELOPT_EXOPL: u8 = 255;

/// TTYPE subnegotiation commands (RFC 1091)
pub const TTYPE_IS: u8 = 0;
pub const TTYPE_SEND: u8 = 1;

/// NAWS subnegotiation (RFC 1073)
pub const NAWS_DATA_SIZE: usize = 4;

/// Authentication option commands (RFC 2941)
pub const AUTH_IS: u8 = 0;
pub const AUTH_SEND: u8 = 1;
pub const AUTH_REPLY: u8 = 2;
pub const AUTH_NAME: u8 = 3;

/// Authentication types (RFC 2941)
pub const AUTH_TYPE_NULL: u8 = 0;
pub const AUTH_TYPE_KERBEROS: u8 = 2;
pub const AUTH_TYPE_SRP: u8 = 5;
pub const AUTH_TYPE_RSA: u8 = 6;

/// Authentication modifiers
pub const AUTH_MOD_WHO_MASK: u8 = 0x01;
pub const AUTH_MOD_CLIENT_TO_SERVER: u8 = 0x00;
pub const AUTH_MOD_SERVER_TO_CLIENT: u8 = 0x01;
pub const AUTH_MOD_HOW_MASK: u8 = 0x02;
pub const AUTH_MOD_ONE_WAY: u8 = 0x00;
pub const AUTH_MOD_MUTUAL: u8 = 0x02;

/// Defaults
pub const DEFAULT_PORT: u16 = 23;
pub const DEFAULT_TIMEOUT: u32 = 30;
pub const RECV_BUFFER_SIZE: usize = 8192;
pub const SEND_BUFFER_SIZE: usize = 4096;

/// Rlogin protocol constants
pub const RLOGIN_NUL: u8 = 0;
