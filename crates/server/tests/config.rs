use lol_coach_server::config::Config;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn base() -> String {
    format!("data_dir = \"/tmp/x\"\ntokens = [\"{TOKEN}\"]\n")
}

fn error_of(text: &str) -> String {
    match Config::parse(text) {
        Ok(_) => panic!("accepted:\n{text}"),
        Err(err) => err.to_string(),
    }
}

#[test]
fn defaults_apply() {
    let config = Config::parse(&base()).unwrap();
    assert_eq!(config.listen.to_string(), "127.0.0.1:8787");
    assert_eq!((config.lookup_ttl_hours, config.max_file_mb), (12, 32));
    assert!(config.riot_key_file.is_none() && config.crawl.is_none());
}

#[test]
fn full_file_parses() {
    let text = format!(
        "{}listen = \"0.0.0.0:9000\"\nriot_key_file = \"/etc/key\"\nlookup_ttl_hours = 1\nmax_file_mb = 5\n[crawl]\nservers = [\"euw1/europe\", \"na1/americas\", \"kr/asia\"]\nmax_games = 100\nfit_at = 10\nrefit_every = 5\npython = \"python3\"\ntools_dir = \"/opt/t\"\n",
        base()
    );
    let config = Config::parse(&text).unwrap();
    assert_eq!(config.listen.port(), 9000);
    assert_eq!(config.crawl.unwrap().servers.len(), 3);
}

#[test]
fn tokens_are_required_and_long_enough() {
    assert!(error_of("data_dir = \"/x\"\n").contains("tokens"));
    assert!(error_of("data_dir = \"/x\"\ntokens = []\n").contains("tokens"));
    let short = error_of("data_dir = \"/x\"\ntokens = [\"short-secret-value\"]\n");
    assert!(short.contains("tokens") && short.contains("32"), "{short}");
    assert!(!short.contains("short-secret-value"));
    let second = error_of(&format!("data_dir = \"/x\"\ntokens = [\"{TOKEN}\", \"tiny\"]\n"));
    assert!(second.contains("token 2"), "{second}");
}

#[test]
fn a_bad_value_names_its_key_and_hides_the_value() {
    let err = error_of(&format!("data_dir = \"/x\"\ntokens = [\"{TOKEN}\"]\nlisten = \"not-an-address\"\n"));
    assert!(err.contains("listen"), "{err}");
    assert!(!err.contains("not-an-address"));
    let err = error_of(&format!("tokens = [\"{TOKEN}\"]\n"));
    assert!(err.contains("data_dir"), "{err}");
    let err = error_of(&format!("{}max_file_mb = 0\n", base()));
    assert!(err.contains("max_file_mb"), "{err}");
    let err = error_of(&format!("{}tokens_typo = 1\n", base()));
    assert!(err.contains("tokens_typo"), "{err}");
}

#[test]
fn a_secret_in_the_wrong_place_is_not_echoed() {
    let err = error_of(&format!("data_dir = \"/x\"\ntokens = \"{TOKEN}\"\n"));
    assert!(err.contains("tokens"), "{err}");
    assert!(!err.contains(TOKEN), "{err}");
}

fn crawl(body: &str) -> String {
    format!("{}[crawl]\ntools_dir = \"/opt/t\"\n{body}", base())
}

#[test]
fn crawl_servers_are_checked() {
    assert!(Config::parse(&crawl("servers = [\"euw1/europe\"]\n")).is_ok());
    assert!(error_of(&crawl("servers = []\n")).contains("crawl.servers"));
    assert!(error_of(&crawl("servers = [\"euw1\"]\n")).contains("crawl.servers"));
    assert!(error_of(&crawl("servers = [\"euw1/asia\"]\n")).contains("crawl.servers"));
    assert!(error_of(&crawl("servers = [\"nowhere/europe\"]\n")).contains("crawl.servers"));
    let two = error_of(&crawl("servers = [\"euw1/europe\", \"eun1/europe\"]\n"));
    assert!(two.contains("crawl.servers") && two.contains("europe"), "{two}");
}

#[test]
fn crawl_numbers_are_checked() {
    for key in ["max_games", "fit_at", "refit_every"] {
        let err = error_of(&crawl(&format!("servers = [\"kr/asia\"]\n{key} = 0\n")));
        assert!(err.contains(&format!("crawl.{key}")), "{err}");
    }
    assert!(error_of(&format!("{}[crawl]\nservers = [\"kr/asia\"]\n", base())).contains("tools_dir"));
}

#[test]
fn load_reports_a_missing_file() {
    let Err(err) = Config::load(std::path::Path::new("/definitely/not/here.toml")) else { panic!("loaded") };
    let err = err.to_string();
    assert!(err.contains("here.toml"), "{err}");
}

#[test]
fn claim_age_defaults_to_a_day_and_must_be_above_zero() {
    assert_eq!(Config::parse(&base()).unwrap().claim_max_age_hours, 24);
    assert_eq!(Config::parse(&format!("{}claim_max_age_hours = 6\n", base())).unwrap().claim_max_age_hours, 6);
    let err = error_of(&format!("{}claim_max_age_hours = 0\n", base()));
    assert!(err.contains("claim_max_age_hours"), "{err}");
}
