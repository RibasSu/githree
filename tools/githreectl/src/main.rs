use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use toml_edit::{DocumentMut, Item, Table, Value, value};

const DEFAULT_SERVICE_NAME: &str = "githree";
const DEFAULT_DEPLOY_MODE: &str = "image";
const DEFAULT_IMAGE_REF: &str = "ghcr.io/sarahsec/githree:latest";

#[derive(Debug, Clone)]
struct ToolConfig {
    project_dir: PathBuf,
    compose_file: PathBuf,
    service_name: String,
    deploy_mode: String,
    image_ref: String,
    githree_config_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerMode {
    Auto,
    Docker,
    SudoDocker,
}

#[derive(Debug, Clone)]
struct GlobalOptions {
    runner_mode: RunnerMode,
    config_path: Option<PathBuf>,
    command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigValueType {
    Auto,
    String,
    Bool,
    Int,
    Float,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("githreectl: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = parse_global_options(args)?;

    if options.command.is_empty() || matches!(options.command[0].as_str(), "help" | "--help" | "-h")
    {
        print_help();
        return Ok(());
    }

    let command = options.command[0].as_str();
    let require_compose = !matches!(command, "config" | "ui");
    let config = ToolConfig::load(options.config_path.clone(), require_compose)?;

    match command {
        "status" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_status(&config, &docker_prefix, &options.command[1..])
        }
        "logs" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_logs(&config, &docker_prefix, &options.command[1..])
        }
        "up" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_up(&config, &docker_prefix, &options.command[1..])
        }
        "down" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_down(&config, &docker_prefix, &options.command[1..])
        }
        "restart" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_restart(&config, &docker_prefix, &options.command[1..])
        }
        "repo" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_repo(&config, &docker_prefix, &options.command[1..])
        }
        "backup" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_backup(&config, &docker_prefix, &options.command[1..])
        }
        "update" => {
            let docker_prefix = detect_docker_prefix(options.runner_mode)?;
            cmd_update(&config, &docker_prefix, &options.command[1..])
        }
        "config" => cmd_config(&config, options.runner_mode, &options.command[1..]),
        "ui" => cmd_ui(&config, options.runner_mode, &options.command[1..]),
        other => Err(format!("unknown command `{other}`\n\n{}", short_usage())),
    }
}

fn short_usage() -> &'static str {
    "Usage: githreectl [--runner <auto|docker|sudo-docker>] [--config <path>] <status|logs|up|down|restart|repo|backup|update|config|ui> [options]"
}

fn print_help() {
    println!(
        "githreectl — host CLI for managing a Githree install\n\
         \n\
         {}\n\
         \n\
         Global options:\n\
           --runner <mode>              Runner mode: auto (default), docker, sudo-docker\n\
           --config <path>              Path to githreectl config env file\n\
         \n\
         Commands:\n\
           status                        Show compose status\n\
           logs [--follow] [service]     Show service logs (default: githree)\n\
           up [--build|--no-build]       Start stack\n\
           down                          Stop stack\n\
           restart                       Restart githree service\n\
           repo add --url <url> [--name <alias>]\n\
           repo remove --name <alias>\n\
           repo fetch --name <alias>\n\
           repo list\n\
           backup [--output <file.tar.gz>]\n\
           update [--backup] [--output <file.tar.gz>]\n\
           config path\n\
           config show\n\
           config get <key>\n\
           config set <key> <value> [--type <auto|string|bool|int|float>] [--restart]\n\
           config unset <key> [--restart]\n\
           ui status\n\
           ui repo-controls <show|hide> [--restart]\n\
           ui web-management <enable|disable> [--restart]\n\
         \n\
         Environment:\n\
           GITHREECTL_CONFIG             Path to config file (default: ~/.config/githreectl/config.env)\n\
         ",
        short_usage()
    );
}

fn parse_global_options(args: Vec<String>) -> Result<GlobalOptions, String> {
    let mut runner_mode = RunnerMode::Auto;
    let mut config_path: Option<PathBuf> = None;
    let mut index = 0usize;

    while index < args.len() {
        let arg = args[index].as_str();

        if arg == "--runner" {
            index += 1;
            if index >= args.len() {
                return Err("missing value for --runner".to_string());
            }
            runner_mode = parse_runner_mode(&args[index])?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--runner=") {
            runner_mode = parse_runner_mode(value)?;
            index += 1;
            continue;
        }

        if arg == "--config" {
            index += 1;
            if index >= args.len() {
                return Err("missing value for --config".to_string());
            }
            config_path = Some(PathBuf::from(&args[index]));
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--config=") {
            config_path = Some(PathBuf::from(value));
            index += 1;
            continue;
        }

        break;
    }

    Ok(GlobalOptions {
        runner_mode,
        config_path,
        command: args[index..].to_vec(),
    })
}

fn parse_runner_mode(raw: &str) -> Result<RunnerMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(RunnerMode::Auto),
        "docker" => Ok(RunnerMode::Docker),
        "sudo-docker" | "sudo" => Ok(RunnerMode::SudoDocker),
        other => Err(format!(
            "invalid runner mode `{other}`. Use: auto, docker, sudo-docker"
        )),
    }
}

impl ToolConfig {
    fn load(config_path_override: Option<PathBuf>, require_compose: bool) -> Result<Self, String> {
        let cwd = env::current_dir()
            .map_err(|err| format!("failed to detect current directory: {err}"))?;
        let default_config_path = default_config_path();

        let mut cfg = ToolConfig {
            project_dir: cwd.clone(),
            compose_file: cwd.join(".run/install/docker-compose.install.yml"),
            service_name: DEFAULT_SERVICE_NAME.to_string(),
            deploy_mode: DEFAULT_DEPLOY_MODE.to_string(),
            image_ref: DEFAULT_IMAGE_REF.to_string(),
            githree_config_file: cwd.join("config/default.toml"),
        };

        let config_path = config_path_override
            .or_else(|| env::var("GITHREECTL_CONFIG").ok().map(PathBuf::from))
            .unwrap_or(default_config_path);

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).map_err(|err| {
                format!(
                    "failed to read config file {}: {err}",
                    config_path.display()
                )
            })?;
            let base_dir = config_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            cfg.apply_config_content(&content, &base_dir)?;
        }

        if cfg.service_name.trim().is_empty() {
            cfg.service_name = DEFAULT_SERVICE_NAME.to_string();
        }

        if cfg.deploy_mode.trim().is_empty() {
            cfg.deploy_mode = DEFAULT_DEPLOY_MODE.to_string();
        }

        if cfg.image_ref.trim().is_empty() {
            cfg.image_ref = DEFAULT_IMAGE_REF.to_string();
        }

        if cfg.githree_config_file.to_string_lossy().trim().is_empty() {
            cfg.githree_config_file = cfg.project_dir.join("config/default.toml");
        }

        if require_compose && !cfg.compose_file.exists() {
            return Err(format!(
                "compose file not found at {}. Run install.sh first or set GITHREECTL_CONFIG.",
                cfg.compose_file.display()
            ));
        }

        Ok(cfg)
    }

    fn apply_config_content(&mut self, content: &str, base_dir: &Path) -> Result<(), String> {
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("invalid config line: `{line}`"))?;
            let key = key.trim();
            let value = value.trim();

            match key {
                "project_dir" => {
                    self.project_dir = resolve_config_path(value, base_dir);
                }
                "compose_file" => {
                    self.compose_file = resolve_config_path(value, base_dir);
                }
                "service_name" => {
                    self.service_name = value.to_string();
                }
                "deploy_mode" => {
                    self.deploy_mode = value.to_string();
                }
                "image_ref" => {
                    self.image_ref = value.to_string();
                }
                "githree_config_file" => {
                    self.githree_config_file = resolve_config_path(value, base_dir);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

fn default_config_path() -> PathBuf {
    if let Ok(path) = env::var("HOME") {
        return PathBuf::from(path).join(".config/githreectl/config.env");
    }
    PathBuf::from(".githreectl-config.env")
}

fn resolve_config_path(value: &str, base_dir: &Path) -> PathBuf {
    let expanded = expand_home(value);
    if expanded.is_absolute() {
        return expanded;
    }
    base_dir.join(expanded)
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~"
        && let Ok(home) = env::var("HOME")
    {
        return PathBuf::from(home);
    }

    if let Some(stripped) = value.strip_prefix("~/")
        && let Ok(home) = env::var("HOME")
    {
        return PathBuf::from(home).join(stripped);
    }

    PathBuf::from(value)
}

fn detect_docker_prefix(mode: RunnerMode) -> Result<Vec<String>, String> {
    if !command_exists("docker") {
        return Err("`docker` is not installed or not in PATH".to_string());
    }

    match mode {
        RunnerMode::Docker => Ok(vec!["docker".to_string()]),
        RunnerMode::SudoDocker => {
            if command_exists("sudo") {
                Ok(vec!["sudo".to_string(), "docker".to_string()])
            } else {
                Err("runner mode `sudo-docker` requires `sudo` in PATH".to_string())
            }
        }
        RunnerMode::Auto => detect_docker_prefix_auto(),
    }
}

fn detect_docker_prefix_auto() -> Result<Vec<String>, String> {
    let probe = Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("failed to execute `docker info`: {err}"))?;

    if probe.status.success() {
        return Ok(vec!["docker".to_string()]);
    }

    let stderr = String::from_utf8_lossy(&probe.stderr);
    let stderr_lc = stderr.to_ascii_lowercase();
    if stderr_lc.contains("permission denied while trying to connect to the docker api") {
        if command_exists("sudo") {
            return Ok(vec!["sudo".to_string(), "docker".to_string()]);
        }
        return Err(
            "docker socket access is denied and `sudo` is unavailable. Add your user to the docker group or run as root."
                .to_string(),
        );
    }

    Err(format!("docker is unavailable: {}", stderr.trim()))
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn compose_prefix(config: &ToolConfig, docker_prefix: &[String]) -> Vec<String> {
    let mut command = docker_prefix.to_vec();
    command.push("compose".to_string());
    command.push("-f".to_string());
    command.push(path_to_string(&config.compose_file));
    command
}

fn run_passthrough(command: &[String]) -> Result<(), String> {
    if command.is_empty() {
        return Err("internal error: empty command".to_string());
    }

    let status = Command::new(&command[0])
        .args(&command[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| format!("failed to execute `{}`: {err}", display_command(command)))?;

    if status.success() {
        return Ok(());
    }

    Err(format!("command failed: {}", display_command(command)))
}

fn display_command(command: &[String]) -> String {
    command
        .iter()
        .map(|segment| shell_escape(segment))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn cmd_status(
    config: &ToolConfig,
    docker_prefix: &[String],
    args: &[String],
) -> Result<(), String> {
    if !args.is_empty() {
        return Err("`status` does not accept arguments".to_string());
    }

    let mut command = compose_prefix(config, docker_prefix);
    command.push("ps".to_string());
    run_passthrough(&command)
}

fn cmd_logs(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
    let mut follow = false;
    let mut service = config.service_name.clone();

    for arg in args {
        match arg.as_str() {
            "--follow" | "-f" => follow = true,
            value if value.starts_with('-') => return Err(format!("unknown logs flag: {value}")),
            value => service = value.to_string(),
        }
    }

    let mut command = compose_prefix(config, docker_prefix);
    command.push("logs".to_string());
    if follow {
        command.push("-f".to_string());
    }
    command.push(service);
    run_passthrough(&command)
}

fn cmd_up(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
    let mut build = config.deploy_mode.eq_ignore_ascii_case("build");

    for arg in args {
        match arg.as_str() {
            "--build" => build = true,
            "--no-build" => build = false,
            value => return Err(format!("unknown up flag: {value}")),
        }
    }

    let mut command = compose_prefix(config, docker_prefix);
    command.push("up".to_string());
    command.push("-d".to_string());
    if build {
        command.push("--build".to_string());
    }
    run_passthrough(&command)
}

fn cmd_down(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("`down` does not accept arguments".to_string());
    }

    let mut command = compose_prefix(config, docker_prefix);
    command.push("down".to_string());
    run_passthrough(&command)
}

fn cmd_restart(
    config: &ToolConfig,
    docker_prefix: &[String],
    args: &[String],
) -> Result<(), String> {
    if !args.is_empty() {
        return Err("`restart` does not accept arguments".to_string());
    }

    let mut command = compose_prefix(config, docker_prefix);
    command.push("restart".to_string());
    command.push(config.service_name.clone());
    run_passthrough(&command)
}

fn cmd_repo(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("missing repo subcommand. Use add/remove/fetch/list".to_string());
    }

    match args[0].as_str() {
        "add" => {
            let parsed = parse_named_options(&args[1..], &["--url", "-u", "--name", "-n"])?;
            let url = parsed
                .get("--url")
                .cloned()
                .ok_or_else(|| "`repo add` requires --url".to_string())?;
            let mut command = compose_exec_prefix(config, docker_prefix);
            command.extend([
                "githree".to_string(),
                "repo".to_string(),
                "add".to_string(),
                "--url".to_string(),
                url,
            ]);
            if let Some(name) = parsed.get("--name") {
                command.push("--name".to_string());
                command.push(name.clone());
            }
            run_passthrough(&command)
        }
        "remove" => {
            let parsed = parse_named_options(&args[1..], &["--name", "-n"])?;
            let name = parsed
                .get("--name")
                .cloned()
                .ok_or_else(|| "`repo remove` requires --name".to_string())?;
            let mut command = compose_exec_prefix(config, docker_prefix);
            command.extend([
                "githree".to_string(),
                "repo".to_string(),
                "remove".to_string(),
                "--name".to_string(),
                name,
            ]);
            run_passthrough(&command)
        }
        "fetch" => {
            let parsed = parse_named_options(&args[1..], &["--name", "-n"])?;
            let name = parsed
                .get("--name")
                .cloned()
                .ok_or_else(|| "`repo fetch` requires --name".to_string())?;
            let mut command = compose_exec_prefix(config, docker_prefix);
            command.extend([
                "githree".to_string(),
                "repo".to_string(),
                "fetch".to_string(),
                "--name".to_string(),
                name,
            ]);
            run_passthrough(&command)
        }
        "list" => {
            if args.len() != 1 {
                return Err("`repo list` does not accept extra arguments".to_string());
            }
            let mut command = compose_exec_prefix(config, docker_prefix);
            command.extend([
                "githree".to_string(),
                "repo".to_string(),
                "list".to_string(),
            ]);
            run_passthrough(&command)
        }
        other => Err(format!("unknown repo subcommand: {other}")),
    }
}

fn compose_exec_prefix(config: &ToolConfig, docker_prefix: &[String]) -> Vec<String> {
    let mut command = compose_prefix(config, docker_prefix);
    command.push("exec".to_string());
    command.push("-T".to_string());
    command.push(config.service_name.clone());
    command
}

fn parse_named_options(
    args: &[String],
    allowed_flags: &[&str],
) -> Result<HashMap<String, String>, String> {
    let mut output: HashMap<String, String> = HashMap::new();
    let mut index = 0usize;

    while index < args.len() {
        let flag = args[index].as_str();
        if !allowed_flags.contains(&flag) {
            return Err(format!("unknown argument: {flag}"));
        }
        index += 1;
        if index >= args.len() {
            return Err(format!("missing value for {flag}"));
        }
        let value = args[index].clone();
        match flag {
            "--url" | "-u" => {
                output.insert("--url".to_string(), value);
            }
            "--name" | "-n" => {
                output.insert("--name".to_string(), value);
            }
            "--output" | "-o" => {
                output.insert("--output".to_string(), value);
            }
            _ => {}
        }
        index += 1;
    }

    Ok(output)
}

fn cmd_backup(
    config: &ToolConfig,
    docker_prefix: &[String],
    args: &[String],
) -> Result<(), String> {
    let parsed = parse_named_options(args, &["--output", "-o"])?;
    let output_path = parsed
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_backup_path(config));

    let output = run_backup(config, docker_prefix, &output_path)?;
    println!("Backup written to {}", output.display());
    Ok(())
}

fn cmd_update(
    config: &ToolConfig,
    docker_prefix: &[String],
    args: &[String],
) -> Result<(), String> {
    let mut backup_before_update = false;
    let mut output: Option<PathBuf> = None;
    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "--backup" => {
                backup_before_update = true;
                index += 1;
            }
            "--output" | "-o" => {
                index += 1;
                if index >= args.len() {
                    return Err("missing value for --output".to_string());
                }
                output = Some(PathBuf::from(args[index].clone()));
                index += 1;
            }
            other => return Err(format!("unknown update argument: {other}")),
        }
    }

    if backup_before_update {
        let backup_path = output.unwrap_or_else(|| default_backup_path(config));
        let created_backup = run_backup(config, docker_prefix, &backup_path)?;
        println!("Backup written to {}", created_backup.display());
    }

    if config.deploy_mode.eq_ignore_ascii_case("build") {
        let mut command = compose_prefix(config, docker_prefix);
        command.extend(["up".to_string(), "-d".to_string(), "--build".to_string()]);
        run_passthrough(&command)?;
    } else {
        let mut pull = compose_prefix(config, docker_prefix);
        pull.extend(["pull".to_string(), config.service_name.clone()]);
        run_passthrough(&pull)?;

        let mut up = compose_prefix(config, docker_prefix);
        up.extend([
            "up".to_string(),
            "-d".to_string(),
            config.service_name.clone(),
        ]);
        run_passthrough(&up)?;
    }

    let mut ps = compose_prefix(config, docker_prefix);
    ps.extend(["ps".to_string(), config.service_name.clone()]);
    run_passthrough(&ps)
}

fn cmd_config(config: &ToolConfig, runner_mode: RunnerMode, args: &[String]) -> Result<(), String> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_config_help();
        return Ok(());
    }

    match args[0].as_str() {
        "path" => {
            println!("{}", config.githree_config_file.display());
            Ok(())
        }
        "show" => {
            let doc = read_githree_config_doc(config)?;
            println!("# {}", config.githree_config_file.display());
            if doc.is_empty() {
                println!("# (empty)");
            } else {
                print!("{}", doc);
            }
            Ok(())
        }
        "get" => {
            if args.len() != 2 {
                return Err("usage: githreectl config get <key>".to_string());
            }
            let doc = read_githree_config_doc(config)?;
            let item = get_dotted_item(&doc, &args[1]).ok_or_else(|| {
                format!(
                    "config key `{}` was not found in {}",
                    args[1],
                    config.githree_config_file.display()
                )
            })?;
            println!("{}", item_display(item));
            Ok(())
        }
        "set" => cmd_config_set(config, runner_mode, &args[1..]),
        "unset" => cmd_config_unset(config, runner_mode, &args[1..]),
        other => Err(format!("unknown config subcommand: {other}")),
    }
}

fn print_config_help() {
    println!(
        "Usage:\n  githreectl config path\n  githreectl config show\n  githreectl config get <key>\n  githreectl config set <key> <value> [--type <auto|string|bool|int|float>] [--restart]\n  githreectl config unset <key> [--restart]"
    );
}

fn cmd_config_set(
    config: &ToolConfig,
    runner_mode: RunnerMode,
    args: &[String],
) -> Result<(), String> {
    if args.len() < 2 {
        return Err(
            "usage: githreectl config set <key> <value> [--type <auto|string|bool|int|float>] [--restart]"
                .to_string(),
        );
    }

    let key = args[0].clone();
    let raw_value = args[1].clone();
    let mut value_type = ConfigValueType::Auto;
    let mut restart = false;
    let mut index = 2usize;

    while index < args.len() {
        match args[index].as_str() {
            "--restart" => {
                restart = true;
                index += 1;
            }
            "--type" => {
                index += 1;
                if index >= args.len() {
                    return Err("missing value for --type".to_string());
                }
                value_type = parse_config_value_type(&args[index])?;
                index += 1;
            }
            other => return Err(format!("unknown config set option: {other}")),
        }
    }

    let mut doc = read_githree_config_doc(config)?;
    let item = toml_item_from_cli_value(&raw_value, value_type)?;
    set_dotted_item(&mut doc, &key, item)?;
    write_githree_config_doc(config, &doc)?;
    println!(
        "Updated {} in {}",
        key,
        config.githree_config_file.display()
    );

    if restart {
        restart_service_with_runner(config, runner_mode)?;
        println!("Service restarted: {}", config.service_name);
    }

    Ok(())
}

fn cmd_config_unset(
    config: &ToolConfig,
    runner_mode: RunnerMode,
    args: &[String],
) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: githreectl config unset <key> [--restart]".to_string());
    }

    let key = args[0].clone();
    let mut restart = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--restart" => restart = true,
            other => return Err(format!("unknown config unset option: {other}")),
        }
    }

    let mut doc = read_githree_config_doc(config)?;
    let removed = remove_dotted_item(&mut doc, &key)?;
    if !removed {
        return Err(format!("config key `{key}` was not found"));
    }

    write_githree_config_doc(config, &doc)?;
    println!(
        "Removed {} from {}",
        key,
        config.githree_config_file.display()
    );

    if restart {
        restart_service_with_runner(config, runner_mode)?;
        println!("Service restarted: {}", config.service_name);
    }

    Ok(())
}

fn cmd_ui(config: &ToolConfig, runner_mode: RunnerMode, args: &[String]) -> Result<(), String> {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_ui_help();
        return Ok(());
    }

    match args[0].as_str() {
        "status" => {
            let doc = read_githree_config_doc(config)?;
            let show_controls = get_bool_with_default(&doc, "features.show_repo_controls", true);
            let web_management = get_bool_with_default(&doc, "features.web_repo_management", false);
            println!("features.show_repo_controls = {show_controls}");
            println!("features.web_repo_management = {web_management}");
            Ok(())
        }
        "repo-controls" => {
            if args.len() < 2 {
                return Err(
                    "usage: githreectl ui repo-controls <show|hide> [--restart]".to_string()
                );
            }
            let value = match args[1].as_str() {
                "show" => true,
                "hide" => false,
                other => {
                    return Err(format!("invalid value `{other}`. Use `show` or `hide`"));
                }
            };
            let restart = parse_restart_flag(&args[2..])?;
            set_bool_key(config, "features.show_repo_controls", value)?;
            println!("features.show_repo_controls = {value}");
            if restart {
                restart_service_with_runner(config, runner_mode)?;
                println!("Service restarted: {}", config.service_name);
            }
            Ok(())
        }
        "web-management" => {
            if args.len() < 2 {
                return Err(
                    "usage: githreectl ui web-management <enable|disable> [--restart]".to_string(),
                );
            }
            let value = match args[1].as_str() {
                "enable" => true,
                "disable" => false,
                other => {
                    return Err(format!(
                        "invalid value `{other}`. Use `enable` or `disable`"
                    ));
                }
            };
            let restart = parse_restart_flag(&args[2..])?;
            set_bool_key(config, "features.web_repo_management", value)?;
            println!("features.web_repo_management = {value}");
            if restart {
                restart_service_with_runner(config, runner_mode)?;
                println!("Service restarted: {}", config.service_name);
            }
            Ok(())
        }
        other => Err(format!("unknown ui subcommand: {other}")),
    }
}

fn print_ui_help() {
    println!(
        "Usage:\n  githreectl ui status\n  githreectl ui repo-controls <show|hide> [--restart]\n  githreectl ui web-management <enable|disable> [--restart]"
    );
}

fn parse_restart_flag(args: &[String]) -> Result<bool, String> {
    let mut restart = false;
    for arg in args {
        match arg.as_str() {
            "--restart" => restart = true,
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok(restart)
}

fn restart_service_with_runner(config: &ToolConfig, runner_mode: RunnerMode) -> Result<(), String> {
    if !config.compose_file.exists() {
        return Err(format!(
            "compose file not found at {}. Cannot restart service.",
            config.compose_file.display()
        ));
    }
    let docker_prefix = detect_docker_prefix(runner_mode)?;
    let mut command = compose_prefix(config, &docker_prefix);
    command.push("restart".to_string());
    command.push(config.service_name.clone());
    run_passthrough(&command)
}

fn read_githree_config_doc(config: &ToolConfig) -> Result<DocumentMut, String> {
    let path = &config.githree_config_file;
    if !path.exists() {
        return Ok(DocumentMut::new());
    }

    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(DocumentMut::new());
    }

    content
        .parse::<DocumentMut>()
        .map_err(|err| format!("failed to parse TOML file {}: {err}", path.display()))
}

fn write_githree_config_doc(config: &ToolConfig, doc: &DocumentMut) -> Result<(), String> {
    let path = &config.githree_config_file;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create config dir {}: {err}", parent.display()))?;
    }

    fs::write(path, doc.to_string())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn split_dotted_key(key: &str) -> Result<Vec<&str>, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("config key cannot be empty".to_string());
    }

    let parts = trimmed.split('.').map(str::trim).collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(format!("invalid dotted key `{key}`"));
    }
    Ok(parts)
}

fn get_dotted_item<'a>(doc: &'a DocumentMut, key: &str) -> Option<&'a Item> {
    let parts = split_dotted_key(key).ok()?;
    let mut table = doc.as_table();

    for (index, part) in parts.iter().enumerate() {
        let item = table.get(part)?;
        if index + 1 == parts.len() {
            return Some(item);
        }
        table = item.as_table()?;
    }

    None
}

fn set_dotted_item(doc: &mut DocumentMut, key: &str, item: Item) -> Result<(), String> {
    let parts = split_dotted_key(key)?;
    let last = parts
        .last()
        .ok_or_else(|| "config key cannot be empty".to_string())?
        .to_string();

    if parts.len() == 1 {
        doc.as_table_mut().insert(&last, item);
        return Ok(());
    }

    let mut table = doc.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        let exists = table.get(part);
        if let Some(existing_item) = exists {
            if !existing_item.is_table() {
                return Err(format!(
                    "cannot set `{key}` because `{part}` is not a table in {}",
                    key
                ));
            }
        } else {
            table.insert(part, Item::Table(Table::new()));
        }

        let next = table
            .get_mut(part)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| format!("failed to access table segment `{part}` for key `{key}`"))?;
        table = next;
    }

    table.insert(&last, item);
    Ok(())
}

fn remove_dotted_item(doc: &mut DocumentMut, key: &str) -> Result<bool, String> {
    let parts = split_dotted_key(key)?;
    let last = parts
        .last()
        .ok_or_else(|| "config key cannot be empty".to_string())?
        .to_string();

    if parts.len() == 1 {
        return Ok(doc.as_table_mut().remove(&last).is_some());
    }

    let mut table = doc.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        let Some(item) = table.get_mut(part) else {
            return Ok(false);
        };
        let Some(next) = item.as_table_mut() else {
            return Ok(false);
        };
        table = next;
    }

    Ok(table.remove(&last).is_some())
}

fn item_display(item: &Item) -> String {
    if let Some(v) = item.as_value() {
        return v.to_string();
    }
    if item.is_table() {
        return "<table>".to_string();
    }
    if item.is_array_of_tables() {
        return "<array-of-tables>".to_string();
    }
    "<none>".to_string()
}

fn parse_config_value_type(raw: &str) -> Result<ConfigValueType, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(ConfigValueType::Auto),
        "string" => Ok(ConfigValueType::String),
        "bool" | "boolean" => Ok(ConfigValueType::Bool),
        "int" | "integer" => Ok(ConfigValueType::Int),
        "float" => Ok(ConfigValueType::Float),
        other => Err(format!(
            "invalid value type `{other}`. Use auto|string|bool|int|float"
        )),
    }
}

fn parse_bool_literal(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(format!("invalid boolean value `{other}`. Use true/false")),
    }
}

fn looks_like_float(raw: &str) -> bool {
    let normalized = raw.trim().to_ascii_lowercase();
    normalized.contains('.') || normalized.contains('e')
}

fn toml_item_from_cli_value(raw: &str, value_type: ConfigValueType) -> Result<Item, String> {
    match value_type {
        ConfigValueType::String => Ok(value(raw.to_string())),
        ConfigValueType::Bool => Ok(value(parse_bool_literal(raw)?)),
        ConfigValueType::Int => {
            let parsed = raw
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("invalid integer value `{raw}`"))?;
            Ok(value(parsed))
        }
        ConfigValueType::Float => {
            let parsed = raw
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("invalid float value `{raw}`"))?;
            Ok(value(parsed))
        }
        ConfigValueType::Auto => {
            if let Ok(boolean) = parse_bool_literal(raw) {
                return Ok(value(boolean));
            }
            if let Ok(integer) = raw.trim().parse::<i64>() {
                return Ok(value(integer));
            }
            if looks_like_float(raw)
                && let Ok(float_value) = raw.trim().parse::<f64>()
            {
                return Ok(value(float_value));
            }
            Ok(value(raw.to_string()))
        }
    }
}

fn get_bool_with_default(doc: &DocumentMut, key: &str, default_value: bool) -> bool {
    get_dotted_item(doc, key)
        .and_then(Item::as_value)
        .and_then(Value::as_bool)
        .unwrap_or(default_value)
}

fn set_bool_key(config: &ToolConfig, key: &str, value_bool: bool) -> Result<(), String> {
    let mut doc = read_githree_config_doc(config)?;
    set_dotted_item(&mut doc, key, value(value_bool))?;
    write_githree_config_doc(config, &doc)
}

fn default_backup_path(config: &ToolConfig) -> PathBuf {
    let ts = unix_timestamp();
    config
        .project_dir
        .join(".run/install/backups")
        .join(format!("githree-backup-{ts}.tar.gz"))
}

fn run_backup(
    config: &ToolConfig,
    docker_prefix: &[String],
    output_path: &Path,
) -> Result<PathBuf, String> {
    let work_dir = config.project_dir.join(".run/install/tmp").join(format!(
        "backup-{}-{}",
        unix_timestamp(),
        std::process::id()
    ));

    let backup_data = work_dir.join("data");
    let backup_config = work_dir.join("config");

    let backup_result = (|| -> Result<(), String> {
        fs::create_dir_all(&backup_data)
            .map_err(|err| format!("failed to create backup data directory: {err}"))?;
        fs::create_dir_all(&backup_config)
            .map_err(|err| format!("failed to create backup config directory: {err}"))?;

        let mut copy_data = docker_prefix.to_vec();
        copy_data.push("cp".to_string());
        copy_data.push(format!("{}:/app/data/.", config.service_name));
        copy_data.push(path_to_string(&backup_data));
        run_passthrough(&copy_data)?;

        let mut copy_config = docker_prefix.to_vec();
        copy_config.push("cp".to_string());
        copy_config.push(format!("{}:/app/config/.", config.service_name));
        copy_config.push(path_to_string(&backup_config));
        run_passthrough(&copy_config)?;

        fs::create_dir_all(
            output_path
                .parent()
                .ok_or_else(|| "invalid backup output path".to_string())?,
        )
        .map_err(|err| format!("failed to create backup output directory: {err}"))?;

        fs::write(
            work_dir.join("metadata.txt"),
            format!(
                "timestamp={}\nservice_name={}\ndeploy_mode={}\nimage_ref={}\ncompose_file={}\n",
                unix_timestamp(),
                config.service_name,
                config.deploy_mode,
                config.image_ref,
                config.compose_file.display()
            ),
        )
        .map_err(|err| format!("failed to write backup metadata: {err}"))?;

        if !command_exists("tar") {
            return Err("`tar` is required to create backup archives".to_string());
        }

        let tar_command = vec![
            "tar".to_string(),
            "-czf".to_string(),
            path_to_string(output_path),
            "-C".to_string(),
            path_to_string(&work_dir),
            ".".to_string(),
        ];
        run_passthrough(&tar_command)?;

        Ok(())
    })();

    let _ = fs::remove_dir_all(&work_dir);
    backup_result.map(|_| output_path.to_path_buf())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_named_options_handles_aliases() {
        let args = vec![
            "--url".to_string(),
            "https://example.com/repo.git".to_string(),
            "-n".to_string(),
            "sample".to_string(),
        ];
        let parsed =
            parse_named_options(&args, &["--url", "-u", "--name", "-n"]).expect("parse options");
        assert_eq!(
            parsed.get("--url").expect("url"),
            "https://example.com/repo.git"
        );
        assert_eq!(parsed.get("--name").expect("name"), "sample");
    }

    #[test]
    fn parse_named_options_rejects_unknown_flags() {
        let args = vec!["--bad".to_string(), "value".to_string()];
        let err = parse_named_options(&args, &["--url", "-u"]).expect_err("must fail");
        assert!(err.contains("unknown argument"));
    }

    #[test]
    fn resolve_config_path_expands_relative_and_home() {
        let base = PathBuf::from("/tmp/config-dir");
        let relative = resolve_config_path("nested/file.txt", &base);
        assert_eq!(relative, PathBuf::from("/tmp/config-dir/nested/file.txt"));

        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let expanded = resolve_config_path("~/githreectl.conf", &base);
        assert_eq!(expanded, PathBuf::from(home).join("githreectl.conf"));
    }

    #[test]
    fn shell_escape_wraps_unsafe_values() {
        assert_eq!(shell_escape("abc-123_/"), "abc-123_/");
        assert_eq!(shell_escape("contains space"), "'contains space'");
    }

    #[test]
    fn parse_global_options_supports_runner_and_config() {
        let parsed = parse_global_options(vec![
            "--runner".to_string(),
            "sudo-docker".to_string(),
            "--config".to_string(),
            "/tmp/githreectl.env".to_string(),
            "repo".to_string(),
            "list".to_string(),
        ])
        .expect("parse options");

        assert_eq!(parsed.runner_mode, RunnerMode::SudoDocker);
        assert_eq!(
            parsed.config_path,
            Some(PathBuf::from("/tmp/githreectl.env"))
        );
        assert_eq!(parsed.command, vec!["repo".to_string(), "list".to_string()]);
    }

    #[test]
    fn toml_item_from_cli_value_auto_parses_common_types() {
        assert_eq!(
            toml_item_from_cli_value("true", ConfigValueType::Auto)
                .expect("bool")
                .as_value()
                .and_then(Value::as_bool),
            Some(true)
        );

        assert_eq!(
            toml_item_from_cli_value("42", ConfigValueType::Auto)
                .expect("int")
                .as_value()
                .and_then(Value::as_integer),
            Some(42)
        );

        assert_eq!(
            toml_item_from_cli_value("3.14", ConfigValueType::Auto)
                .expect("float")
                .as_value()
                .and_then(Value::as_float),
            Some(3.14)
        );

        assert_eq!(
            toml_item_from_cli_value("hello", ConfigValueType::Auto)
                .expect("string")
                .as_value()
                .and_then(Value::as_str),
            Some("hello")
        );
    }

    #[test]
    fn dotted_item_round_trip_works() {
        let mut doc = DocumentMut::new();
        set_dotted_item(&mut doc, "features.web_repo_management", value(false)).expect("set key");
        set_dotted_item(&mut doc, "features.show_repo_controls", value(true)).expect("set key");

        assert_eq!(
            get_dotted_item(&doc, "features.web_repo_management")
                .and_then(Item::as_value)
                .and_then(Value::as_bool),
            Some(false)
        );

        assert!(remove_dotted_item(&mut doc, "features.web_repo_management").expect("remove"));
        assert!(get_dotted_item(&doc, "features.web_repo_management").is_none());
    }
}
