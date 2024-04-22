use clap::{App};

pub struct Args {
    pub create_message: bool,
    pub code_registration: Option<String>,
    pub receiver_encryption_pk: Option<String>,
    pub recipient: Option<String>,
    pub other: Option<String>,
    pub sender_subidentity: Option<String>,
    pub receiver_subidentity: Option<String>,
    pub inbox: Option<String>,
    pub body_content: Option<String>,
}

pub fn parse_args() -> Args {
    let matches = App::new("Shinkai Node")
        .version(env!("CARGO_PKG_VERSION"))
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
        .arg(clap::Arg::new("other").short('o').long("other").takes_value(true))
        .arg(
            clap::Arg::new("sender_subidentity")
                .short('s')
                .long("sender_subidentity")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("receiver_subidentity")
                .short('a')
                .long("receiver_subidentity")
                .takes_value(true),
        )
        .arg(clap::Arg::new("inbox").short('i').long("inbox").takes_value(true))
        .arg(
            clap::Arg::new("body_content")
                .short('b')
                .long("body_content")
                .takes_value(true),
        )
        .get_matches();

    Args {
        create_message: matches.is_present("create_message"),
        code_registration: matches.value_of("code_registration").map(String::from),
        receiver_encryption_pk: matches.value_of("receiver_encryption_pk").map(String::from),
        recipient: matches.value_of("recipient").map(String::from),
        other: matches.value_of("other").map(String::from),
        sender_subidentity: matches.value_of("sender_subidentity").map(String::from),
        receiver_subidentity: matches.value_of("receiver_subidentity").map(String::from),
        inbox: matches.value_of("inbox").map(String::from),
        body_content: matches.value_of("body_content").map(String::from),
    }
}
