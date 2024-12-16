use clap::{arg, command, value_parser, Args, Command, Parser};
use std::{env, error::Error, io::Write, process::ExitCode, time::Duration};
use tokio::{io::{self, AsyncBufReadExt, AsyncRead, BufReader, Lines}, time::sleep};

use r2con::{RconAuthError, RconClient};

const DEFAULT_PORT: i32 = 25575;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Cli {
    /// RCON server hostname
    #[arg(short = 'H', long)]
    host: Option<String>,

    /// RCON server password
    #[arg(short = 'P', long)]
    password: Option<String>,

    /// Supress output
    #[arg(short, long, default_value_t = false)]
    silent: bool,

    /// Wait time between commands in seconds
    /// (only affects non-interactive mode)
    #[arg(short, long, default_value_t = 0.0)]
    wait_time: f64,

    /// Enable interactive mode after commands are finished
    #[arg(short, long, default_value_t = false)]
    interactive: bool,

    /// commands to run
    commands:Vec<String>
}

struct InputReader<T: AsyncRead> {
    line_reader: Lines<BufReader<T>>,
}

impl<T: AsyncRead + Unpin> InputReader<T> {
    fn new(stream: T) -> InputReader<T> {
        let reader = BufReader::new(stream);
        let lines = reader.lines();
        return InputReader { line_reader: lines };
    }

    async fn readline(&mut self, msg: &str) -> Result<Option<String>, std::io::Error> {
        print!("{}", msg);
        let _ = std::io::stdout().flush();
        let result = self.line_reader.next_line().await;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                eprintln!("Stdin Error: {}", e);
                result
            }
        }
    }

    async fn get_input(&mut self, msg: &str) -> Result<String, Box<dyn Error>> {
        loop {
            let line_opt = self.readline(msg).await?;
            if let Some(line) = line_opt {
                return Ok(line);
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Command::new("r2con")
        .arg(arg!(-p --port <PORT> "RCON port[default: 25575]").value_parser(value_parser!(i32)));
    let cli = Cli::augment_args(cli);

    let matches = cli.get_matches();

    let hostname = get_hostname(matches.get_one::<String>("host").cloned()).await;
    let password = get_password(matches.get_one::<String>("password").cloned()).await;
    let port = get_port(matches.get_one::<i32>("port").cloned());

    let commands = matches.get_many::<String>("commands");
    let commands = if let Some(commands) = commands {
        commands.cloned().collect::<Vec<String>>()
    } else {
        Vec::new()
    };

    let silent = matches.get_one::<bool>("silent").cloned().unwrap();
    let wait_time = matches.get_one::<f64>("wait_time").cloned().unwrap();
    let mut interactive = matches.get_one::<bool>("interactive").cloned().unwrap();

    let addr = if let Ok(hostname) = hostname {
        hostname + ":" + &port.to_string()
    } else {
        if !silent {
            eprintln!("error: no hostname could be read");
        }
        return ExitCode::FAILURE;
    };

    let password = if let Ok(password) = password {
        password
    } else {
        if !silent {
            eprintln!("error: no password could be read");
        }
        return ExitCode::FAILURE;
    };

    let client = RconClient::connect(addr, &password).await;

    return match client {
        Ok(mut rcon_client) => {
            if commands.is_empty() {
                interactive = true;
            }
            let command_loop_result = command_loop(&mut rcon_client, &commands, silent, wait_time).await;
            let result = if let Ok(_) = command_loop_result {
                if interactive {
                    if !silent {
                        if let Ok(addr) = rcon_client.get_address() {
                            println!("Connected to {}", addr.to_string());
                        }
                        println!("Type 'quit' to close.");
                    }
                    interactive_command_loop(&mut rcon_client, silent).await 
                } else {
                    command_loop_result
                }
            } else {
                command_loop_result
            };
            
            if let Err(_) = result {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(ref e) if e.is::<RconAuthError>() => {
            if !silent {
                eprintln!("wrong password: {}", e);
            }
            ExitCode::FAILURE
        }
        Err(e) => {
            if !silent {
                eprintln!("connection Error: {}", e);
            }
            ExitCode::FAILURE
        }
    };
}

async fn run_command(rcon_client: &mut RconClient, command:&str, silent:bool)-> Result<(), Box<dyn Error>>{
    let result = rcon_client.send_command(command).await;
    match result {
        Ok(output) => {
            if !output.is_empty() && !silent {
                println!("{}", output);
            }
        }
        Err(e) => {
            if !silent {
                eprintln!("An error occured while sending the command:");
                eprintln!("Error: {}", e);
            }
            return Err(e.into());
        }
    }
    Ok(())
}

async fn command_loop(rcon_client: &mut RconClient, commands: &Vec<String>, silent:bool, wait_time:f64) -> Result<(), Box<dyn Error>>{
    let command_count = commands.len();
    for (i, command) in commands.iter().enumerate() {
        if let Err(e) = run_command(rcon_client, command, silent).await {
            return Err(e);
        }
        if i != command_count-1 {
            sleep(Duration::from_secs_f64(wait_time)).await;
        }
    }
    Ok(())
}

async fn interactive_command_loop(rcon_client: &mut RconClient, silent:bool) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut reader = InputReader::new(stdin);
    loop {
        let line = reader.get_input("> ").await?;
        let trimmed_line = line.trim();
        if trimmed_line == "quit" {
            break;
        } else if !trimmed_line.is_empty() {
            run_command(rcon_client, trimmed_line, silent).await?
        }
    }
    Ok(())
}

async fn get_hostname(arg: Option<String>) -> Result<String, Box<dyn Error>> {
    let stdin = io::stdin();
    let mut reader = InputReader::new(stdin);
    if let Some(hostname) = arg {
        Ok(hostname)
    } else {
        if let Ok(hostname) = env::var("R2CON_HOST") {
            Ok(hostname)
        } else {
            reader.get_input("Hostname: ").await
        }
    }
}

fn get_port(arg: Option<i32>) -> i32 {
    if let Some(port) = arg {
        port
    } else {
        if let Ok(port_str) = env::var("R2CON_PORT") {
            if let Ok(port) = port_str.parse::<i32>() {
                port
            } else {
                DEFAULT_PORT
            }
        } else {
            DEFAULT_PORT
        }
    }
}

async fn get_password(arg: Option<String>) -> Result<String, Box<dyn Error>> {
    let stdin = io::stdin();
    let mut reader = InputReader::new(stdin);
    if let Some(password) = arg {
        Ok(password)
    } else {
        if let Ok(hostname) = env::var("R2CON_PASS") {
            Ok(hostname)
        } else {
            reader.get_input("Password: ").await
        }
    }
}