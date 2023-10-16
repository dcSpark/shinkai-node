use clap::{App, Arg, ArgMatches};

pub fn setup_cli() -> ArgMatches<'static> {
    App::new("Shinkai")
        .version("1.0")
        .author("Nico <nico@shinkai.com>, Rob <rob@shinkai.com>")
        .about("Shinkai Node Application")
        .arg(
            Arg::with_name("NODE_PROXY_MODE")
                .short("p")
                .long("proxy-mode")
                .value_name("MODE")
                .help("Sets the node proxy mode")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ALLOW_NEW_IDENTITIES")
                .short("a")
                .long("allow-new-identities")
                .value_name("FLAG")
                .help("Sets the flag indicating if new identities can be added")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("PROXY_API_PEER")
                .short("api")
                .long("api-peer")
                .value_name("ADDRESS")
                .help("Sets the address of the API proxy")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("PROXY_TCP_PEER")
                .short("tcp")
                .long("tcp-peer")
                .value_name("ADDRESS")
                .help("Sets the address of the TCP proxy")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("SHINKAI_NAME")
                .short("n")
                .long("name")
                .value_name("NAME")
                .help("Sets the Shinkai name")
                .takes_value(true),
        )
        .get_matches()
}