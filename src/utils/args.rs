// This can be in a new args.rs file
use clap::{App, Arg};

pub struct Args {
    pub create_message: bool,
    pub code_registration: Option<String>,
    pub receiver_encryption_pk: Option<String>,
    pub recipient: Option<String>,
    pub other: Option<String>,
}

pub fn parse_args() -> Args {
    let matches = App::new("Shinkai Node")
        .version("1.0")
        .arg(
            clap::Arg::new("create_message")
                .short('c')
                .long("create_message")
                .takes_value(false),
        )
        .arg(
            clap::Arg::new("code_registration")
                .short('d')
                .long("code_registration")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("receiver_encryption_pk")
                .short('e')
                .long("receiver_encryption_pk")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("recipient")
                .short('r')
                .long("recipient")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("other")
                .short('o')
                .long("other")
                .takes_value(true),
        )
        .get_matches();

    Args {
        create_message: matches.is_present("create_message"),
        code_registration: matches.value_of("code_registration").map(String::from),
        receiver_encryption_pk: matches.value_of("receiver_encryption_pk").map(String::from),
        recipient: matches.value_of("recipient").map(String::from),
        other: matches.value_of("other").map(String::from),
    }
}
