use crate::{
    config::{self, Config},
    ui::Mode,
};
use atty;
use std::{error::Error, fmt, result::Result};

#[derive(Debug)]
pub struct ArgError {
    details: String,
}

impl ArgError {
    pub fn new(err: impl fmt::Display) -> ArgError {
        ArgError {
            details: format!("{}", err),
        }
    }
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ArgError {
    fn description(&self) -> &str {
        &self.details
    }
}

/// Parse command line arguments into a Config structure.
pub fn parse<T: AsRef<str>>(args: &[T]) -> Result<Config, ArgError> {
    let mut cfg = if config::exists() {
        match config::load() {
            Err(e) => return Err(ArgError::new(e)),
            Ok(c) => c,
        }
    } else {
        config::default()
    };

    let mut iter = args.iter();
    let mut got_url = false;
    let mut set_tls = false;
    let mut set_notls = false;
    let mut set_tor = false;
    let mut set_notor = false;
    let mut set_cfg = false;
    let mut set_nocfg = false;
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "-v" | "--version" | "-version" => {
                cfg.mode = Mode::Version;
                return Ok(cfg);
            }
            "-h" | "--help" | "-help" => {
                cfg.mode = Mode::Help;
                return Ok(cfg);
            }
            "-r" | "--raw" | "-raw" => {
                if args.len() > 1 {
                    cfg.mode = Mode::Raw;
                } else {
                    return Err(ArgError::new("--raw needs gopher-url"));
                }
            }
            "-p" | "--print" | "-print" => cfg.mode = Mode::Print,
            "-l" | "--local" | "-local" => cfg.start = "gopher://127.0.0.1:7070".into(),
            "-C" | "--no-config" | "-no-config" => {
                if set_cfg {
                    return Err(ArgError::new("can't mix --config and --no-config"));
                }
                set_nocfg = true;
                cfg = config::default();
            }
            "-c" | "--config" | "-config" => {
                if set_nocfg {
                    return Err(ArgError::new("can't mix --config and --no-config"));
                }
                set_cfg = true;
                if let Some(arg) = iter.next() {
                    cfg = match config::load_file(arg.as_ref()) {
                        Ok(c) => c,
                        Err(e) => {
                            return Err(ArgError::new(format!("error loading config: {}", e)));
                        }
                    };
                } else {
                    return Err(ArgError::new("need a config file"));
                }
            }
            arg if arg.starts_with("--config=") || arg.starts_with("-config=") => {
                if set_nocfg {
                    return Err(ArgError::new("can't mix --config and --no-config"));
                }
                set_cfg = true;
                let mut parts = arg.splitn(2, '=');
                if let Some(file) = parts.nth(1) {
                    cfg = match config::load_file(file) {
                        Ok(c) => c,
                        Err(e) => {
                            return Err(ArgError::new(format!("error loading config: {}", e)));
                        }
                    };
                } else {
                    return Err(ArgError::new("need a config file"));
                }
            }
            "-s" | "--tls" | "-tls" => {
                if set_notls {
                    return Err(ArgError::new("can't set both --tls and --no-tls"));
                }
                set_tls = true;
                cfg.tls = true;
                if cfg!(feature = "disable-tls") {
                    return Err(ArgError::new("phetch was compiled without TLS support"));
                }
            }
            "-S" | "--no-tls" | "-no-tls" => {
                if set_tls {
                    return Err(ArgError::new("can't set both --tls and --no-tls"));
                }
                set_notls = true;
                cfg.tls = false;
            }
            "-o" | "--tor" | "-tor" => {
                if set_notor {
                    return Err(ArgError::new("can't set both --tor and --no-tor"));
                }
                set_tor = true;
                cfg.tor = true;
            }
            "-O" | "--no-tor" | "-no-tor" => {
                if set_tor {
                    return Err(ArgError::new("can't set both --tor and --no-tor"));
                }
                set_notor = true;
                cfg.tor = false;
            }
            arg => {
                if arg.starts_with('-') {
                    return Err(ArgError::new(format!("unknown flag: {}", arg)));
                } else if got_url {
                    return Err(ArgError::new(format!("unknown argument: {}", arg)));
                } else {
                    got_url = true;
                    cfg.start = arg.into();
                }
            }
        }
    }

    if cfg.tor && cfg.tls {
        return Err(ArgError::new("can't set both --tor and --tls"));
    }

    if !atty::is(atty::Stream::Stdout) {
        cfg.mode = Mode::NoTTY;
    }

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let cfg = parse(&["-l"]).expect("failed to parse");
        assert_eq!(cfg.start, "gopher://127.0.0.1:7070");
        assert_eq!(cfg.wide, false);
    }

    #[test]
    fn test_unknown() {
        let err = parse(&["-z"]).expect_err("-z shouldn't exist");
        assert_eq!(err.to_string(), "unknown flag: -z");

        let err = parse(&["-l", "-x"]).expect_err("-x shouldn't exist");
        assert_eq!(err.to_string(), "unknown flag: -x");

        let err = parse(&["sdf.org", "sdf2.org"]).expect_err("two urls should fail");
        assert_eq!(err.to_string(), "unknown argument: sdf2.org");
    }

    #[test]
    fn test_local() {
        let cfg = parse(&["--local"]).expect("should work");
        assert_eq!(cfg.start, "gopher://127.0.0.1:7070");

        let cfg = parse(&["-s", "-l"]).expect("should work");
        assert_eq!(cfg.start, "gopher://127.0.0.1:7070");
        assert_eq!(cfg.tls, true);
    }

    #[test]
    fn test_raw() {
        let cfg = parse(&["--raw", "sdf.org"]).expect("should work");
        assert_eq!(cfg.mode, Mode::Raw);
        assert_eq!(cfg.start, "sdf.org");

        let err = parse(&["--raw"]).expect_err("should fail");
        assert_eq!(err.to_string(), "--raw needs gopher-url");
    }

    #[test]
    fn test_print() {
        let cfg = parse(&["--print", "sdf.org"]).expect("should work");
        assert_eq!(cfg.mode, Mode::Print);
        assert_eq!(cfg.start, "sdf.org");
        let _ = parse(&["--print"]).expect("should work");
        assert_eq!(cfg.mode, Mode::Print);
    }

    #[test]
    fn test_help() {
        let cfg = parse(&["--help"]).expect("should work");
        assert_eq!(cfg.mode, Mode::Help);
    }

    #[test]
    fn test_version() {
        let cfg = parse(&["--version"]).expect("should work");
        assert_eq!(cfg.mode, Mode::Version);
    }

    #[test]
    fn test_tls_tor() {
        let err = parse(&["--tls", "--tor"]).expect_err("should fail");
        assert_eq!(err.to_string(), "can\'t set both --tor and --tls");

        let err = parse(&["--tls", "--no-tls"]).expect_err("should fail");
        assert_eq!(err.to_string(), "can\'t set both --tls and --no-tls");
        let err = parse(&["-s", "-S"]).expect_err("should fail");
        assert_eq!(err.to_string(), "can\'t set both --tls and --no-tls");

        let cfg = parse(&["--tor", "--no-tls"]).expect("should work");
        assert_eq!(cfg.tor, true);
        assert_eq!(cfg.tls, false);
    }

    #[test]
    fn test_config() {
        let err = parse(&["-c"]).expect_err("should fail");
        assert_eq!(err.to_string(), "need a config file");

        let err = parse(&["-C", "-c", "file.conf"]).expect_err("should fail");
        assert_eq!(err.to_string(), "can't mix --config and --no-config");

        let err = parse(&["-c", "file.conf"]).expect_err("should fail");
        assert_eq!(
            err.to_string(),
            "error loading config: No such file or directory (os error 2)"
        );

        let err = parse(&["--config=file.conf"]).expect_err("should fail");
        assert_eq!(
            err.to_string(),
            "error loading config: No such file or directory (os error 2)"
        );

        let err = parse(&["--config", "file.conf"]).expect_err("should fail");
        assert_eq!(
            err.to_string(),
            "error loading config: No such file or directory (os error 2)"
        );

        let cfg = parse(&["-C"]).expect("should work");
        assert_eq!(cfg.tls, false);
    }
}