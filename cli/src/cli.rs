//! Gistit command line interface

use clap::{crate_authors, crate_description, crate_version, App, Arg, SubCommand};

/// The gistit application
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn app() -> App<'static, 'static> {
    App::new("Gistit")
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("colorschemes")
                .long("colorschemes")
                .help("List avaiable colorschemes"),
        )
        .arg(
            Arg::with_name("silent")
                .long("silent")
                .help("Silent mode, omit stdout")
                .global(true),
        )
        .subcommand(
            SubCommand::with_name("send")
                .about("Send the gistit to the cloud")
                .arg(
                    Arg::with_name("file")
                        .long("file")
                        .short("f")
                        .help("The file to be sent [required]")
                        .required(true)
                        .multiple(false) // currently not supported
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("description")
                        .long("description")
                        .short("d")
                        .help("With a description")
                        .takes_value(true)
                        .requires("file"),
                )
                .arg(
                    Arg::with_name("author")
                        .long("author")
                        .short("a")
                        .help("With author information")
                        .takes_value(true)
                        .requires("file"),
                )
                .arg(
                    Arg::with_name("lifespan")
                        .long("lifespan")
                        .short("l")
                        .help("With a custom lifespan")
                        .requires("file")
                        .takes_value(true)
                        .default_value("3600"),
                )
                .arg(
                    Arg::with_name("secret")
                        .long("secret")
                        .short("s")
                        .help("With password encryption")
                        .takes_value(true)
                        .requires("file"),
                )
                .arg(
                    Arg::with_name("theme")
                        .long("theme")
                        .short("t")
                        .default_value("atomDark")
                        .requires("file")
                        .takes_value(true)
                        .help("The colorscheme to apply syntax highlighting")
                        .long_help(
                            "The colorscheme to apply syntax highlighting.
Run `gistit --colorschemes` to list avaiable ones.",
                        ),
                )
                .arg(
                    Arg::with_name("clipboard")
                        .long("clipboard")
                        .short("c")
                        .requires("file")
                        .help("Copies the result hash to the system clipboard")
                        .long_help(
                            "Copies the result hash to the system clipboard.
This program will attempt to find a suitable clipboard program in your system and use it.
If none was found it defaults to ANSI escape sequence OSC52.
This is our best efforts at persisting the hash into the system clipboard after the program exits.
",
                        ),
                )
                .arg(
                    Arg::with_name("dry-run")
                        .long("dry-run")
                        .requires("file")
                        .short("r")
                        .help("Executes gistit-send in 'dry run' mode"),
                ),
        )
        .subcommand(
            SubCommand::with_name("fetch")
                .about("Fetch a gistit by it's hash")
                .arg(
                    Arg::with_name("hash")
                        .short("h")
                        .help("Fetch a gistit via it's hash")
                        .required_unless("url")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("url")
                        .short("u")
                        .required_unless("hash")
                        .help("Attempt to open the gistit on your default browser")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("secret")
                        .short("s")
                        .help("The secret to decrypt the fetched gistit")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("theme")
                        .long("theme")
                        .short("t")
                        .default_value("atomDark")
                        .requires("file")
                        .takes_value(true)
                        .help("The colorscheme to apply syntax highlighting")
                        .long_help(
                            "The colorscheme to apply syntax highlighting.
Run `gistit --colorschemes` to list avaiable ones.",
                        ),
                )
                .arg(Arg::with_name("no-syntax-highlighting").help("Without syntax highlighting")),
        )
}
