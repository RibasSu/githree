use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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
}

fn main() {
    if let Err(err) = run() {
        eprintln!("githreectl: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    let config = ToolConfig::load()?;
    let docker_prefix = detect_docker_prefix()?;

    match args[0].as_str() {
        "status" => cmd_status(&config, &docker_prefix, &args[1..]),
        "logs" => cmd_logs(&config, &docker_prefix, &args[1..]),
        "up" => cmd_up(&config, &docker_prefix, &args[1..]),
        "down" => cmd_down(&config, &docker_prefix, &args[1..]),
        "restart" => cmd_restart(&config, &docker_prefix, &args[1..]),
        "repo" => cmd_repo(&config, &docker_prefix, &args[1..]),
        "backup" => cmd_backup(&config, &docker_prefix, &args[1..]),
        "update" => cmd_update(&config, &docker_prefix, &args[1..]),
        other => Err(format!(
            "unknown command `{other}`\n\n{}",
            short_usage()
        )),
    }
}

fn short_usage() -> &'static str {
    "Usage: githreectl <status|logs|up|down|restart|repo|backup|update> [options]"
}

fn print_help() {
    println!(
        "githreectl — host CLI for managing a Githree install\n\
         \n\
         {}\n\
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
         \n\
         Environment:\n\
           GITHREECTL_CONFIG             Path to config file (default: ~/.config/githreectl/config.env)\n\
         " ,
        short_usage()
    );
}

impl ToolConfig {
    fn load() -> Result<Self, String> {
        let cwd = env::current_dir().map_err(|err| format!("failed to detect current directory: {err}"))?;
        let default_config_path = default_config_path();

        let mut cfg = ToolConfig {
            project_dir: cwd.clone(),
            compose_file: cwd.join(".run/install/docker-compose.install.yml"),
            service_name: DEFAULT_SERVICE_NAME.to_string(),
            deploy_mode: DEFAULT_DEPLOY_MODE.to_string(),
            image_ref: DEFAULT_IMAGE_REF.to_string(),
        };

        let config_path = env::var("GITHREECTL_CONFIG")
            .map(PathBuf::from)
            .unwrap_or(default_config_path);

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .map_err(|err| format!("failed to read config file {}: {err}", config_path.display()))?;
            let base_dir = config_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            cfg.apply_config_content(&content, &base_dir)?;
        }

        if !cfg.compose_file.exists() {
            return Err(format!(
                "compose file not found at {}. Run install.sh first or set GITHREECTL_CONFIG.",
                cfg.compose_file.display()
            ));
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

fn detect_docker_prefix() -> Result<Vec<String>, String> {
    if !command_exists("docker") {
        return Err("`docker` is not installed or not in PATH".to_string());
    }

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

fn cmd_status(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
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

fn cmd_restart(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
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
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut output: std::collections::HashMap<String, String> = std::collections::HashMap::new();
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

fn cmd_backup(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
    let parsed = parse_named_options(args, &["--output", "-o"])?;
    let output_path = parsed
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_backup_path(config));

    let output = run_backup(config, docker_prefix, &output_path)?;
    println!("Backup written to {}", output.display());
    Ok(())
}

fn cmd_update(config: &ToolConfig, docker_prefix: &[String], args: &[String]) -> Result<(), String> {
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
        pull.extend([
            "pull".to_string(),
            config.service_name.clone(),
        ]);
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

fn default_backup_path(config: &ToolConfig) -> PathBuf {
    let ts = unix_timestamp();
    config
        .project_dir
        .join(".run/install/backups")
        .join(format!("githree-backup-{ts}.tar.gz"))
}

fn run_backup(config: &ToolConfig, docker_prefix: &[String], output_path: &Path) -> Result<PathBuf, String> {
    let work_dir = config
        .project_dir
        .join(".run/install/tmp")
        .join(format!("backup-{}-{}", unix_timestamp(), std::process::id()));

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
        let parsed = parse_named_options(&args, &["--url", "-u", "--name", "-n"])
            .expect("parse options");
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
}
