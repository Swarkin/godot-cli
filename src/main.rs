use std::{env, fs, io, process};
use std::cmp::Ordering;
use std::io::{stdin, stdout, Write};
use std::path::Path;

use colored::Colorize;
use serde::{Deserialize, Serialize};

const NAME: &str = "godot-cli";
const CONFIG: &str = "config";

#[derive(Default, Debug, Serialize, Deserialize)]
struct Config {
    godot_exec: String,
    project_dir: String
}


fn main() {
    let mut config: Config;
    match confy::load::<Config>(NAME, CONFIG) {
        Ok(c) => config = c,
        Err(e) => {
            print_config_error(e);
            if prompt("reset to default?", None) {
                config = Config::default();
            } else {
                return;
            }
        }
    }

    let mut args = env::args().skip(1).collect::<Vec<String>>();
    if args.is_empty() { args.push(String::from("help")); }
    let arg_len = args.len();

    args.retain(|arg| {
        match arg.as_str() {
            "--no-color" => { colored::control::set_override(false); }
            "--force-color" => { colored::control::set_override(true); }
            _ => {
                return if arg.starts_with("--") {
                    warn_msg(&format!("unknown arg {}", arg.bold()));
                    false
                } else {
                    true
                }
            }
        }
        false
    });

    let action = args[0].as_str();
    match action {
        "help" | "/?" => print_action_help(),
        "new" | "create" => {
            if !args_count(2, arg_len, Ordering::Equal) { return; }

            if config.godot_exec.is_empty() || config.project_dir.is_empty() {
                print_missing_config_notice(vec!("godot_exec", "project_dir"));
                return;
            }

            let name = &args[1];
            if !is_valid_name(name) { return; }

            let project_dir = format!("{}/{name}", config.project_dir);

            if !prompt(&format!("confirm {} of project \"{}\"?", "creation".cyan().bold(), project_dir.bold()), None) { return; }

            match fs::create_dir(&project_dir) {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == io::ErrorKind::AlreadyExists {
                        println!("error: project \"{name}\" already exists");
                        return;
                    }
                }
            }

            {
                let mut file = fs::File::create(format!("{project_dir}/project.godot")).unwrap();
                file.write_all(format!("[application]\n\nconfig/name=\"{name}\"").as_bytes())
                    .unwrap();
            }

            open_godot(vec!("-e", "--path", &project_dir));
        }
        "open" => {
            if !args_count(2, arg_len, Ordering::Equal) { return; }

            if config.godot_exec.is_empty() || config.project_dir.is_empty() {
                print_missing_config_notice(vec!("godot_exec", "project_dir"));
                return;
            }

            let name = &args[1];
            if !is_valid_name(name) { return; }

            let project_dir = format!("{}/{name}", config.project_dir);
            let path = Path::new(&project_dir);
            if !path.exists() || path.is_file() {
                err_msg("invalid path or no permission");
                return;
            }

            println!("opening project {}...", project_dir.bold());

            open_godot(vec!("-e", "--path", &project_dir));
        }
        "run" => {
            if !args_count(1, arg_len, Ordering::Greater) { return; }
            if !args_count(4, arg_len, Ordering::Less) { return; }

            if config.godot_exec.is_empty() || config.project_dir.is_empty() {
                print_missing_config_notice(vec!("godot_exec", "project_dir"));
                return;
            }

            let name = &args[1];
            if !is_valid_name(name) { return; }

            let instances_string: String;
            let instances: u8;
            if arg_len > 2 {
                instances_string = args[2].clone();
                match instances_string.clone().parse::<u8>() {
                    Ok(v) => instances = v,
                    Err(e) => {
                        warn_msg(&format!("invalid instance count (0-255): {e}. defaulting to 1"));
                        instances = 1;
                    }
                }
            } else {
                instances_string = String::from("1");
                instances = 1;
            }

            let project_dir = format!("{}/{name}", config.project_dir);
            let path = Path::new(&project_dir);
            if !path.exists() || path.is_file() {
                err_msg("invalid path or no permission");
                return;
            }
            
            if instances > 4 && !prompt(&format!("run {} instances of the project?", instances_string.bold()), None) { return; }
            
            println!("running project {} with {instances} instances...", project_dir.bold());

            for _ in 0..instances {
                open_godot(vec!("--path", &project_dir));
            }
        }
        "list" => {
            if !args_count(1, arg_len, Ordering::Equal) { return; }

            if config.project_dir.is_empty() {
                print_missing_config_notice(vec!("project_dir"));
                return;
            }

            for dir in fs::read_dir(&config.project_dir).unwrap() {
                match dir {
                    Ok(entry) => {
                        let mut path = entry.path();

                        let path_meta = path.metadata().unwrap();
                        if path_meta.is_file() { continue; }

                        path.push("project.godot");
                        if !path.is_file() { continue; }

                        println!("{:?}", entry.file_name());

                    }
                    Err(e) => {
                        println!("{e}");
                    }
                }
            }
        }
        "delete" | "remove" => {
            if !args_count(2, arg_len, Ordering::Equal) { return; }

            if config.godot_exec.is_empty() || config.project_dir.is_empty() {
                print_missing_config_notice(vec!("godot_exec", "project_dir"));
                return;
            }

            let name = &args[1];
            if !is_valid_name(name) { return; }

            let project_dir = format!("{}/{name}", config.project_dir);
            if !Path::new(&project_dir).exists() {
                err_msg(&format!("project \"{}\" not found", project_dir.bold()));
                return;
            }

            if !prompt(&format!("confirm {} of \"{project_dir}\"?", "deletion".red().bold()), None) { return; }

            fs::remove_dir_all(project_dir).unwrap()
        }
        "config" => {
            if arg_len == 1 {
                println!("{} {}\n", "location:".green().bold(), confy::get_configuration_file_path(NAME, CONFIG).unwrap().to_string_lossy());
                print_config_help();
                return;
            }

            if !args_count(1, arg_len, Ordering::Greater) { return; }

            let sub_action = args[1].as_str();
            match sub_action {
                "get" => {
                    if !args_count(3, arg_len, Ordering::Equal) { return; }

                    let entry = args[2].as_str();
                    match entry {
                        "godot_exec" => println!("{}", config.godot_exec),
                        "project_dir" => println!("{}", config.project_dir),
                        _ => err_msg(&format!("unknown config entry {}", entry.bold()))
                    }
                }
                "set" => {
                    if !args_count(4, arg_len, Ordering::Equal) { return; }

                    let entry = args[2].as_str();
                    let value = args[3].as_str();
                    match entry {
                        "godot_exec" => {
                            let path = Path::new(value);
                            if !path.exists() || path.is_dir() {
                                err_msg("invalid path or no permission");
                                return;
                            }
                            config.godot_exec = value.to_string();
                        },
                        "project_dir" => {
                            let path = Path::new(value);
                            if !path.exists() && path.is_file() {
                                err_msg("invalid path or no permission");
                                return;
                            }
                            config.project_dir = value.to_string();
                        }
                        _ => err_msg(&format!("unknown config entry {}", entry.bold()))
                    }
                }
                "delete" | "remove" => {
                    if !args_count(3, arg_len, Ordering::Equal) { return; }

                    let entry = args[2].as_str();
                    match entry {
                        "godot_exec" => config.godot_exec.clear(),
                        "project_dir" => config.project_dir.clear(),
                        _ => err_msg(&format!("unknown config entry {}", entry.bold()))
                    }
                }
                "clear" => {
                    if prompt("confirm deletion of config?", None) {
                        config = Config::default();
                    }
                }
                _ => {
                    err_msg(&format!("invalid action {}", sub_action.bold()));
                    return;
                }
            }

            confy::store(NAME, CONFIG, config).unwrap_or_else(|e| {
                err_msg(&format!("failed to save config: {e}"));
            });
        }
        _ => {
            err_msg(&format!("invalid action {}", action.bold()));
        }
    }
}

fn open_godot(args: Vec<&str>) {
    process::Command::new("godot")
        .args(args)
        .spawn()
        .unwrap();
}

fn prompt(msg: &str, cancel_msg: Option<&str>) -> bool {
    print!("{msg} {} ", "(y/n)".bright_black());
    if stdout().flush().is_err() { return false; }

    let mut buf = String::new();
    stdin().read_line(&mut buf).unwrap();
    buf.make_ascii_lowercase();
    
    if buf.trim_end() == "y" {
        true
    } else {
        err_msg(cancel_msg.unwrap_or("canceled"));
        false
    }
}

fn is_valid_name(name: &str) -> bool {
    if !name.is_ascii() {
        err_msg("non-ascii project name");
        false
    } else {
        true
    }
}

fn args_count(num: usize, amnt: usize, ord: Ordering) -> bool {
    //println!("argcount debug: n {num}, actual {amnt}, ord {ord:?}");
    let result = amnt.cmp(&num);
    if result == ord {
        true
    } else {
        let ord_str = match ord {
            Ordering::Less => "less than".bold(),
            Ordering::Equal => "exactly".bold(),
            Ordering::Greater => "more than".bold()
        };
        err_msg(&format!("expected {ord_str} {num} {}, got {amnt}", if num == 1 { "arg" } else { "args" }));
        false
    }
}

fn err_msg(msg: &str) {
    eprintln!("{} {msg}", "error:".red().bold());
}

fn warn_msg(msg: &str) {
    eprintln!("{} {msg}", "warn:".yellow().bold());
}

fn hint_msg(msg: &str) {
    println!("{} {msg}", "hint:".cyan().bold());
}

fn print_action_help() {
    println!("{} - a convenience cli for godot", NAME.green().bold());
    hint_msg(&format!("to force disable/enable the use of colors, use {} respectively\n", "--no-color/--force-color".bold()));

    println!("{} get/set entry [value] | configure the cli", "config".bold());
    println!("{}/{} name | create a project", "new".bold(), "create".bold());
    println!("{} name | open a project", "open".bold());
    println!("{} name [n] | run a project [n times]", "run".bold());
    println!("{} | list all projects", "list".bold());
    println!("{}/{} name | delete a project\n", "delete".bold(), "remove".bold());
}

fn print_config_help() {
    println!("  {}", "actions:".cyan().bold());
    println!("{}: get a config entry", "get".bold());
    println!("{}: set a config entry", "set".bold());
    println!("{}: clear a config entry", "delete/remove".bold());
    println!("{}: clear the entire config\n", "clear".bold());

    println!("  {}", "entries:".cyan().bold());
    println!("{}: path to the executable", "godot_exec".bold());
    println!("{}: directory containing projects\n", "project_dir".bold());
}

fn print_missing_config_notice(settings: Vec<&str>) {
    warn_msg(&format!("please set up the following config entr{}:\n{}\nby using {}",
            if settings.len() == 1 { "y" } else { "ies"},
            settings.join("\n").bold(),
            "godot-cli config <entry> <value>".bold()
    ));
}

fn print_config_error(e: confy::ConfyError) {
    err_msg(&format!("failed to load config: {e}"));
}
