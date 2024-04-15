#[cfg(test)]
mod tests {
    use std::{fs, path::Path, sync::Arc};

    use super::*;
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use mockito::Server;
    use shinkai_message_primitives::{
        schemas::shinkai_name::ShinkaiName,
        shinkai_utils::{signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair}, shinkai_logging::init_default_tracing},
    };
    use shinkai_node::{
        agent::job_manager::JobManager,
        cron_tasks::{cron_manager::CronManager, web_scrapper::WebScraper},
        db::{db_cron_task::CronTask, ShinkaiDB},
        managers::IdentityManager,
        vector_fs::vector_fs::VectorFS,
    };
    use shinkai_vector_resources::{
        embedding_generator::RemoteEmbeddingGenerator, unstructured::unstructured_api::UnstructuredAPI,
    };
    use tokio::sync::Mutex;
    use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_extract_links() {
        init_default_tracing(); 
        let links = WebScraper::extract_links(&get_unstructured_response());
        assert_eq!(links.len(), 30);
    }

    #[tokio::test]
    #[ignore]
    async fn test_web_scraper() {
        init_default_tracing(); 
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let (identity_secret_key, _) = unsafe_deterministic_signature_keypair(0);
        let node_profile_name = ShinkaiName::new("@@localhost.shinkai/main".to_string()).unwrap();
        // Originals
        // let api_url = "https://internal.shinkai.com/x-unstructured-api/general/v0/general".to_string();
        // let target_website = "https://news.ycombinator.com".to_string();

        let mut target_url_server = Server::new();
        let _m = target_url_server
            .mock("GET", "/")
            .with_status(200)
            .with_header("Content-Type", "text/html")
            .with_body(&get_html_content())
            .create();

        let mut unstructured_server = Server::new();
        let _mm = unstructured_server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&get_unstructured_response())
            .create();

        let task = CronTask {
            task_id: "task1".to_string(),
            cron: "* * * * *".to_string(),
            prompt: "Prompt".to_string(),
            subprompt: "Subprompt".to_string(),
            url: target_url_server.url(),
            crawl_links: true,
            created_at: chrono::Utc::now().to_rfc3339().to_string(),
            agent_id: "agent1".to_string(),
        };

        let scraper = WebScraper {
            task,
            unstructured_api: UnstructuredAPI::new_default(),
        };

        let subidentity_manager = IdentityManager::new(db.clone(), node_profile_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));
        let vector_fs = Arc::new(Mutex::new(VectorFS::new_empty()));

        let job_manager = Arc::new(Mutex::new(
            JobManager::new(
                Arc::clone(&db),
                Arc::clone(&identity_manager),
                clone_signature_secret_key(&identity_secret_key),
                node_profile_name.clone(),
                vector_fs,
                RemoteEmbeddingGenerator::new_default(),
                UnstructuredAPI::new_default(),
            )
            .await,
        ));

        // TODO: mock up the other 30 websites lol
        // TODO: let's modify this so it only returns one link
        let result = CronManager::process_job_message_queued(
            scraper.task.clone(),
            db,
            identity_secret_key,
            job_manager,
            node_profile_name,
            "main_profile".to_string(),
        )
        .await;

        // let result = scraper.download_and_parse().await;
        assert!(result.is_ok());
    }

    fn get_unstructured_response() -> String {
        r###"[{"element_id":"a3c80f9c6ef360f2855105216d0dc806","metadata":{"filename":"_0621e8a7-f27f-42bc-b60a-a4cedd8c07e7.html","filetype":"text/html","languages":["eng"],"page_number": 1,"text_as_html":"_REMOVED_TO_SAVE_SPACE_"},"text":"Hacker News \\\n                             new  |  past  |  comments  |  ask  |  show  |  jobs  |  submit              \\\n                               login \\n                           \\n                \\n             \\n       1.        Omegle founder shuts down site forever?  ( omegle.com ) \\n           261 points  by  liamcottle   1 hour ago    |  hide  |  119\u00A0comments          \\n               \\n       \\n                 \\n       2.        SciPy builds for Python 3.12 on Windows are a minor miracle  ( quansight.org ) \\n           217 points  by  todsacerdoti   5 hours ago    |  hide  |  102\u00A0comments          \\n               \\n       \\n                 \\n       3.        Interesting Bugs Caught by ESLint's no-constant-binary-expression  ( eslint.org ) \\n           110 points  by  CharlesW   5 hours ago    |  hide  |  38\u00A0comments          \\n               \\n       \\n                 \\n       4.        On-Crash Backtraces in Swift  ( swift.org ) \\n           29 points  by  yaglo   2 hours ago    |  hide  |  3\u00A0comments          \\n               \\n       \\n                 \\n       5.        Quake Brutalist Jam II  ( slipseer.com ) \\n           283 points  by  jakearmitage   10 hours ago    |  hide  |  87\u00A0comments          \\n               \\n       \\n                 \\n       6.        Why does unsafe multithreaded std:unordered_map crash more than std:map?  ( microsoft.com ) \\n           11 points  by  luu   1 hour ago    |  hide  |  4\u00A0comments          \\n               \\n       \\n                 \\n       7.        Major outages across ChatGPT and API  ( openai.com ) \\n           432 points  by  d99kris   11 hours ago    |  hide  |  461\u00A0comments          \\n               \\n       \\n                 \\n       8.        Automerge-Repo: A \"batteries-included\" toolkit for local-first applications  ( automerge.org ) \\n           141 points  by  gklitt   8 hours ago    |  hide  |  26\u00A0comments          \\n               \\n       \\n                 \\n       9.        Show HN: Bulletpapers – ArXiv AI paper summarizer, won Anthropic Hackathon  ( bulletpapers.ai ) \\n           86 points  by  mattfalconer   7 hours ago    |  hide  |  20\u00A0comments          \\n               \\n       \\n                 \\n       10.        All About Cats, and What Ethernet Classifications Mean  ( hackaday.com ) \\n           44 points  by  rcarmo   5 hours ago    |  hide  |  20\u00A0comments          \\n               \\n       \\n                 \\n       11.        Sad clown paradox  ( wikipedia.org ) \\n           12 points  by  11001100   1 hour ago    |  hide  |  1\u00A0comment          \\n               \\n       \\n                 \\n       12.        Kurdish Parentheses on OpenStreetMap, Three Ways  ( georeactor.com ) \\n           63 points  by  mapmeld   6 hours ago    |  hide  |  7\u00A0comments          \\n               \\n       \\n                 \\n       13.        Inside the weird and delightful origins of the jungle gym, which just turned 100  ( npr.org ) \\n           69 points  by  geox   7 hours ago    |  hide  |  26\u00A0comments          \\n               \\n       \\n                 \\n       14.        Man crushed to death by robot that mistook him for a box of vegetables  ( telegraph.co.uk ) \\n           44 points  by  ummonk   1 hour ago    |  hide  |  31\u00A0comments          \\n               \\n       \\n                 \\n       15.        Koch Snowflake  ( tikalon.com ) \\n           11 points  by  the-mitr   2 hours ago    |  hide  |  1\u00A0comment          \\n               \\n       \\n                 \\n       16.        Officially Qualified – Ferrocene  ( ferrous-systems.com ) \\n           294 points  by  jamincan   14 hours ago    |  hide  |  98\u00A0comments          \\n               \\n       \\n                 \\n       17.        Voters Overwhelmingly Pass Car Right to Repair Law in Maine  ( 404media.co ) \\n           191 points  by  maxwell   6 hours ago    |  hide  |  42\u00A0comments          \\n               \\n       \\n                 \\n       18.        Home Assistant blocked from integrating with Garage Door opener API  ( home-assistant.io ) \\n           947 points  by  eamonnsullivan   16 hours ago    |  hide  |  536\u00A0comments          \\n               \\n       \\n                 \\n       19.        We wrote the OpenAI Wanderlust app in pure Python using Solara  ( github.com/widgetti ) \\n           56 points  by  maartenbreddels   5 hours ago    |  hide  |  22\u00A0comments          \\n               \\n       \\n                 \\n       20.        Punica: Serving multiple LoRA finetuned LLM as one  ( github.com/punica-ai ) \\n           50 points  by  abcdabcd987   5 hours ago    |  hide  |  7\u00A0comments          \\n               \\n       \\n                 \\n       21.        Fossil Fuel Use Increasing, Not Decreasing  ( nytimes.com ) \\n           5 points  by  lxm   1 hour ago    |  hide  |  discuss          \\n               \\n       \\n                 \\n       22.        TESS discovers Saturn-like planet orbiting an M-dwarf star  ( phys.org ) \\n           28 points  by  wglb   5 hours ago    |  hide  |  4\u00A0comments          \\n               \\n       \\n                 \\n       23.        Signal Public Username Testing (Staging Environment)  ( signalusers.org ) \\n           40 points  by  mindracer   4 hours ago    |  hide  |  40\u00A0comments          \\n               \\n       \\n                 \\n       24.        Evolve Your Hierarchy (2007)  ( cowboyprogramming.com ) \\n           13 points  by  lambda_garden   4 hours ago    |  hide  |  1\u00A0comment          \\n               \\n       \\n                 \\n       25.        Zuckerberg personally rejected Meta's proposals to improve teen mental health  ( cnn.com ) \\n           16 points  by  miguelazo   26 minutes ago    |  hide  |  discuss          \\n               \\n       \\n                 \\n       26.        Opusmodus: Common Lisp Music Composition System  ( opusmodus.com ) \\n           154 points  by  zetalyrae   14 hours ago    |  hide  |  53\u00A0comments          \\n               \\n       \\n                 \\n       27.        Whither philosophy?  ( aeon.co ) \\n           34 points  by  apollinaire   9 hours ago    |  hide  |  70\u00A0comments          \\n               \\n       \\n                 \\n       28.        Original photo from Led Zeppelin IV album cover discovered  ( bbc.com ) \\n           102 points  by  boulos   8 hours ago    |  hide  |  30\u00A0comments          \\n               \\n       \\n                 \\n       29.        Oil and gas production in Texas produces twice as much methane as in New Mexico  ( theguardian.com ) \\n           212 points  by  webmaven   7 hours ago    |  hide  |  86\u00A0comments          \\n               \\n       \\n                 \\n       30.        $200M gift propels scientific research in the search for life beyond Earth  ( seti.org ) \\n           124 points  by  webmaven   6 hours ago    |  hide  |  84\u00A0comments          \\n               \\n       \\n             \\n       More      \\n   \\n \\n Guidelines  |  FAQ  |  Lists  |  API  |  Security  |  Legal  |  Apply to YC  |  Contact \\n Search:", "type": "Table"}]"###.to_string()
    }

    fn get_html_content() -> String {
        r###"<html lang="en" op="news"><head><meta name="referrer" content="origin"><meta name="viewport" content="width=device-width, initial-scale=1.0"><link rel="stylesheet" type="text/css" href="news.css?lkyh8o6g5AgZZ2PaGgEr">
        <link rel="shortcut icon" href="favicon.ico">
          <link rel="alternate" type="application/rss+xml" title="RSS" href="rss">
        <title>Hacker News</title></head><body><center><table id="hnmain" border="0" cellpadding="0" cellspacing="0" width="85%" bgcolor="#f6f6ef">
        <tr><td bgcolor="#ff6600"><table border="0" cellpadding="0" cellspacing="0" width="100%" style="padding:2px"><tr><td style="width:18px;padding-right:4px"><a href="https://news.ycombinator.com"><img src="y18.svg" width="18" height="18" style="border:1px white solid; display:block"></a></td>
                  <td style="line-height:12pt; height:10px;"><span class="pagetop"><b class="hnname"><a href="news">Hacker News</a></b>
                            <a href="newest">new</a> | <a href="front">past</a> | <a href="newcomments">comments</a> | <a href="ask">ask</a> | <a href="show">show</a> | <a href="jobs">jobs</a> | <a href="submit">submit</a>            </span></td><td style="text-align:right;padding-right:4px;"><span class="pagetop">
                              <a href="login?goto=news">login</a>
                          </span></td>
              </tr></table></td></tr>
<tr id="pagespace" title="" style="height:10px"></tr><tr><td><table border="0" cellpadding="0" cellspacing="0">
            <tr class='athing' id='38199355'>
      <td align="right" valign="top" class="title"><span class="rank">1.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38199355'href='vote?id=38199355&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.omegle.com/" rel="noreferrer">Omegle founder shuts down site forever?</a><span class="sitebit comhead"> (<a href="from?site=omegle.com"><span class="sitestr">omegle.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38199355">261 points</span> by <a href="user?id=liamcottle" class="hnuser">liamcottle</a> <span class="age" title="2023-11-09T00:40:25"><a href="item?id=38199355">1 hour ago</a></span> <span id="unv_38199355"></span> | <a href="hide?id=38199355&amp;goto=news">hide</a> | <a href="item?id=38199355">119&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38196412'>
      <td align="right" valign="top" class="title"><span class="rank">2.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38196412'href='vote?id=38196412&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://labs.quansight.org/blog/building-scipy-with-flang" rel="noreferrer">SciPy builds for Python 3.12 on Windows are a minor miracle</a><span class="sitebit comhead"> (<a href="from?site=quansight.org"><span class="sitestr">quansight.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38196412">217 points</span> by <a href="user?id=todsacerdoti" class="hnuser">todsacerdoti</a> <span class="age" title="2023-11-08T20:24:16"><a href="item?id=38196412">5 hours ago</a></span> <span id="unv_38196412"></span> | <a href="hide?id=38196412&amp;goto=news">hide</a> | <a href="item?id=38196412">102&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38196644'>
      <td align="right" valign="top" class="title"><span class="rank">3.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38196644'href='vote?id=38196644&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://eslint.org/blog/2022/07/interesting-bugs-caught-by-no-constant-binary-expression/" rel="noreferrer">Interesting Bugs Caught by ESLint&#x27;s no-constant-binary-expression</a><span class="sitebit comhead"> (<a href="from?site=eslint.org"><span class="sitestr">eslint.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38196644">110 points</span> by <a href="user?id=CharlesW" class="hnuser">CharlesW</a> <span class="age" title="2023-11-08T20:41:34"><a href="item?id=38196644">5 hours ago</a></span> <span id="unv_38196644"></span> | <a href="hide?id=38196644&amp;goto=news">hide</a> | <a href="item?id=38196644">38&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38198638'>
      <td align="right" valign="top" class="title"><span class="rank">4.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38198638'href='vote?id=38198638&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.swift.org/blog/swift-5.9-backtraces/" rel="noreferrer">On-Crash Backtraces in Swift</a><span class="sitebit comhead"> (<a href="from?site=swift.org"><span class="sitestr">swift.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38198638">29 points</span> by <a href="user?id=yaglo" class="hnuser">yaglo</a> <span class="age" title="2023-11-08T23:16:23"><a href="item?id=38198638">2 hours ago</a></span> <span id="unv_38198638"></span> | <a href="hide?id=38198638&amp;goto=news">hide</a> | <a href="item?id=38198638">3&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38191319'>
      <td align="right" valign="top" class="title"><span class="rank">5.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38191319'href='vote?id=38191319&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.slipseer.com/index.php?resources/quake-brutalist-jam-2.278/" rel="noreferrer">Quake Brutalist Jam II</a><span class="sitebit comhead"> (<a href="from?site=slipseer.com"><span class="sitestr">slipseer.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38191319">283 points</span> by <a href="user?id=jakearmitage" class="hnuser">jakearmitage</a> <span class="age" title="2023-11-08T15:02:29"><a href="item?id=38191319">10 hours ago</a></span> <span id="unv_38191319"></span> | <a href="hide?id=38191319&amp;goto=news">hide</a> | <a href="item?id=38191319">87&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38171749'>
      <td align="right" valign="top" class="title"><span class="rank">6.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38171749'href='vote?id=38171749&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://devblogs.microsoft.com/oldnewthing/20231103-00/?p=108966" rel="noreferrer">Why does unsafe multithreaded std:unordered_map crash more than std:map?</a><span class="sitebit comhead"> (<a href="from?site=microsoft.com"><span class="sitestr">microsoft.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38171749">11 points</span> by <a href="user?id=luu" class="hnuser">luu</a> <span class="age" title="2023-11-07T01:08:59"><a href="item?id=38171749">1 hour ago</a></span> <span id="unv_38171749"></span> | <a href="hide?id=38171749&amp;goto=news">hide</a> | <a href="item?id=38171749">4&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38190401'>
      <td align="right" valign="top" class="title"><span class="rank">7.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38190401'href='vote?id=38190401&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://status.openai.com/incidents/00fpy0yxrx1q" rel="noreferrer">Major outages across ChatGPT and API</a><span class="sitebit comhead"> (<a href="from?site=openai.com"><span class="sitestr">openai.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38190401">432 points</span> by <a href="user?id=d99kris" class="hnuser">d99kris</a> <span class="age" title="2023-11-08T14:02:39"><a href="item?id=38190401">11 hours ago</a></span> <span id="unv_38190401"></span> | <a href="hide?id=38190401&amp;goto=news">hide</a> | <a href="item?id=38190401">461&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38193640'>
      <td align="right" valign="top" class="title"><span class="rank">8.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38193640'href='vote?id=38193640&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://automerge.org/blog/2023/11/06/automerge-repo/" rel="noreferrer">Automerge-Repo: A &quot;batteries-included&quot; toolkit for local-first applications</a><span class="sitebit comhead"> (<a href="from?site=automerge.org"><span class="sitestr">automerge.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38193640">141 points</span> by <a href="user?id=gklitt" class="hnuser">gklitt</a> <span class="age" title="2023-11-08T17:19:27"><a href="item?id=38193640">8 hours ago</a></span> <span id="unv_38193640"></span> | <a href="hide?id=38193640&amp;goto=news">hide</a> | <a href="item?id=38193640">26&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38194586'>
      <td align="right" valign="top" class="title"><span class="rank">9.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38194586'href='vote?id=38194586&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.bulletpapers.ai" rel="noreferrer">Show HN: Bulletpapers – ArXiv AI paper summarizer, won Anthropic Hackathon</a><span class="sitebit comhead"> (<a href="from?site=bulletpapers.ai"><span class="sitestr">bulletpapers.ai</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38194586">86 points</span> by <a href="user?id=mattfalconer" class="hnuser">mattfalconer</a> <span class="age" title="2023-11-08T18:20:11"><a href="item?id=38194586">7 hours ago</a></span> <span id="unv_38194586"></span> | <a href="hide?id=38194586&amp;goto=news">hide</a> | <a href="item?id=38194586">20&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38188567'>
      <td align="right" valign="top" class="title"><span class="rank">10.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38188567'href='vote?id=38188567&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://hackaday.com/2023/11/07/all-about-cats-and-what-ethernet-classifications-mean-beyond-bigger-number-better/" rel="noreferrer">All About Cats, and What Ethernet Classifications Mean</a><span class="sitebit comhead"> (<a href="from?site=hackaday.com"><span class="sitestr">hackaday.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38188567">44 points</span> by <a href="user?id=rcarmo" class="hnuser">rcarmo</a> <span class="age" title="2023-11-08T10:18:15"><a href="item?id=38188567">5 hours ago</a></span> <span id="unv_38188567"></span> | <a href="hide?id=38188567&amp;goto=news">hide</a> | <a href="item?id=38188567">20&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38198417'>
      <td align="right" valign="top" class="title"><span class="rank">11.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38198417'href='vote?id=38198417&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://en.wikipedia.org/wiki/Sad_clown_paradox" rel="nofollow noreferrer">Sad clown paradox</a><span class="sitebit comhead"> (<a href="from?site=wikipedia.org"><span class="sitestr">wikipedia.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38198417">12 points</span> by <a href="user?id=11001100" class="hnuser">11001100</a> <span class="age" title="2023-11-08T22:53:30"><a href="item?id=38198417">1 hour ago</a></span> <span id="unv_38198417"></span> | <a href="hide?id=38198417&amp;goto=news">hide</a> | <a href="item?id=38198417">1&nbsp;comment</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38195436'>
      <td align="right" valign="top" class="title"><span class="rank">12.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38195436'href='vote?id=38195436&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://blog.georeactor.com/osm-1" rel="noreferrer">Kurdish Parentheses on OpenStreetMap, Three Ways</a><span class="sitebit comhead"> (<a href="from?site=georeactor.com"><span class="sitestr">georeactor.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38195436">63 points</span> by <a href="user?id=mapmeld" class="hnuser">mapmeld</a> <span class="age" title="2023-11-08T19:14:52"><a href="item?id=38195436">6 hours ago</a></span> <span id="unv_38195436"></span> | <a href="hide?id=38195436&amp;goto=news">hide</a> | <a href="item?id=38195436">7&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38193354'>
      <td align="right" valign="top" class="title"><span class="rank">13.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38193354'href='vote?id=38193354&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.npr.org/2023/11/08/1209932614/jungle-gym-playground-monkey-bars-maths-hinton-fourth-dimension" rel="noreferrer">Inside the weird and delightful origins of the jungle gym, which just turned 100</a><span class="sitebit comhead"> (<a href="from?site=npr.org"><span class="sitestr">npr.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38193354">69 points</span> by <a href="user?id=geox" class="hnuser">geox</a> <span class="age" title="2023-11-08T17:04:02"><a href="item?id=38193354">7 hours ago</a></span> <span id="unv_38193354"></span> | <a href="hide?id=38193354&amp;goto=news">hide</a> | <a href="item?id=38193354">26&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38199233'>
      <td align="right" valign="top" class="title"><span class="rank">14.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38199233'href='vote?id=38199233&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.telegraph.co.uk/world-news/2023/11/08/man-crushed-to-death-south-korea-industrial-robot/" rel="noreferrer">Man crushed to death by robot that mistook him for a box of vegetables</a><span class="sitebit comhead"> (<a href="from?site=telegraph.co.uk"><span class="sitestr">telegraph.co.uk</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38199233">44 points</span> by <a href="user?id=ummonk" class="hnuser">ummonk</a> <span class="age" title="2023-11-09T00:21:14"><a href="item?id=38199233">1 hour ago</a></span> <span id="unv_38199233"></span> | <a href="hide?id=38199233&amp;goto=news">hide</a> | <a href="item?id=38199233">31&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38187445'>
      <td align="right" valign="top" class="title"><span class="rank">15.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38187445'href='vote?id=38187445&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="http://www.tikalon.com/blog/blog.php?article=2023/Koch_snowflake" rel="noreferrer">Koch Snowflake</a><span class="sitebit comhead"> (<a href="from?site=tikalon.com"><span class="sitestr">tikalon.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38187445">11 points</span> by <a href="user?id=the-mitr" class="hnuser">the-mitr</a> <span class="age" title="2023-11-08T06:42:53"><a href="item?id=38187445">2 hours ago</a></span> <span id="unv_38187445"></span> | <a href="hide?id=38187445&amp;goto=news">hide</a> | <a href="item?id=38187445">1&nbsp;comment</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38188734'>
      <td align="right" valign="top" class="title"><span class="rank">16.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38188734'href='vote?id=38188734&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://ferrous-systems.com/blog/officially-qualified-ferrocene/" rel="noreferrer">Officially Qualified – Ferrocene</a><span class="sitebit comhead"> (<a href="from?site=ferrous-systems.com"><span class="sitestr">ferrous-systems.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38188734">294 points</span> by <a href="user?id=jamincan" class="hnuser">jamincan</a> <span class="age" title="2023-11-08T10:49:08"><a href="item?id=38188734">14 hours ago</a></span> <span id="unv_38188734"></span> | <a href="hide?id=38188734&amp;goto=news">hide</a> | <a href="item?id=38188734">98&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38195509'>
      <td align="right" valign="top" class="title"><span class="rank">17.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38195509'href='vote?id=38195509&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.404media.co/voters-overwhelmingly-pass-car-right-to-repair-law-in-maine/" rel="noreferrer">Voters Overwhelmingly Pass Car Right to Repair Law in Maine</a><span class="sitebit comhead"> (<a href="from?site=404media.co"><span class="sitestr">404media.co</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38195509">191 points</span> by <a href="user?id=maxwell" class="hnuser">maxwell</a> <span class="age" title="2023-11-08T19:20:02"><a href="item?id=38195509">6 hours ago</a></span> <span id="unv_38195509"></span> | <a href="hide?id=38195509&amp;goto=news">hide</a> | <a href="item?id=38195509">42&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38188162'>
      <td align="right" valign="top" class="title"><span class="rank">18.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38188162'href='vote?id=38188162&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.home-assistant.io/blog/2023/11/06/removal-of-myq-integration/" rel="noreferrer">Home Assistant blocked from integrating with Garage Door opener API</a><span class="sitebit comhead"> (<a href="from?site=home-assistant.io"><span class="sitestr">home-assistant.io</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38188162">947 points</span> by <a href="user?id=eamonnsullivan" class="hnuser">eamonnsullivan</a> <span class="age" title="2023-11-08T09:04:18"><a href="item?id=38188162">16 hours ago</a></span> <span id="unv_38188162"></span> | <a href="hide?id=38188162&amp;goto=news">hide</a> | <a href="item?id=38188162">536&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38196008'>
      <td align="right" valign="top" class="title"><span class="rank">19.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38196008'href='vote?id=38196008&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://github.com/widgetti/wanderlust">We wrote the OpenAI Wanderlust app in pure Python using Solara</a><span class="sitebit comhead"> (<a href="from?site=github.com/widgetti"><span class="sitestr">github.com/widgetti</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38196008">56 points</span> by <a href="user?id=maartenbreddels" class="hnuser">maartenbreddels</a> <span class="age" title="2023-11-08T19:56:01"><a href="item?id=38196008">5 hours ago</a></span> <span id="unv_38196008"></span> | <a href="hide?id=38196008&amp;goto=news">hide</a> | <a href="item?id=38196008">22&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38196661'>
      <td align="right" valign="top" class="title"><span class="rank">20.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38196661'href='vote?id=38196661&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://github.com/punica-ai/punica">Punica: Serving multiple LoRA finetuned LLM as one</a><span class="sitebit comhead"> (<a href="from?site=github.com/punica-ai"><span class="sitestr">github.com/punica-ai</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38196661">50 points</span> by <a href="user?id=abcdabcd987" class="hnuser">abcdabcd987</a> <span class="age" title="2023-11-08T20:42:32"><a href="item?id=38196661">5 hours ago</a></span> <span id="unv_38196661"></span> | <a href="hide?id=38196661&amp;goto=news">hide</a> | <a href="item?id=38196661">7&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38199304'>
      <td align="right" valign="top" class="title"><span class="rank">21.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38199304'href='vote?id=38199304&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.nytimes.com/2023/11/08/climate/fossil-fuels-expanding.html" rel="nofollow noreferrer">Fossil Fuel Use Increasing, Not Decreasing</a><span class="sitebit comhead"> (<a href="from?site=nytimes.com"><span class="sitestr">nytimes.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38199304">5 points</span> by <a href="user?id=lxm" class="hnuser">lxm</a> <span class="age" title="2023-11-09T00:32:08"><a href="item?id=38199304">1 hour ago</a></span> <span id="unv_38199304"></span> | <a href="hide?id=38199304&amp;goto=news">hide</a> | <a href="item?id=38199304">discuss</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38182732'>
      <td align="right" valign="top" class="title"><span class="rank">22.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38182732'href='vote?id=38182732&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://phys.org/news/2023-11-tess-saturn-like-planet-orbiting-m-dwarf.html" rel="noreferrer">TESS discovers Saturn-like planet orbiting an M-dwarf star</a><span class="sitebit comhead"> (<a href="from?site=phys.org"><span class="sitestr">phys.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38182732">28 points</span> by <a href="user?id=wglb" class="hnuser">wglb</a> <span class="age" title="2023-11-07T20:58:34"><a href="item?id=38182732">5 hours ago</a></span> <span id="unv_38182732"></span> | <a href="hide?id=38182732&amp;goto=news">hide</a> | <a href="item?id=38182732">4&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38197469'>
      <td align="right" valign="top" class="title"><span class="rank">23.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38197469'href='vote?id=38197469&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://community.signalusers.org/t/public-username-testing-staging-environment/56866" rel="noreferrer">Signal Public Username Testing (Staging Environment)</a><span class="sitebit comhead"> (<a href="from?site=signalusers.org"><span class="sitestr">signalusers.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38197469">40 points</span> by <a href="user?id=mindracer" class="hnuser">mindracer</a> <span class="age" title="2023-11-08T21:39:07"><a href="item?id=38197469">4 hours ago</a></span> <span id="unv_38197469"></span> | <a href="hide?id=38197469&amp;goto=news">hide</a> | <a href="item?id=38197469">40&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38175213'>
      <td align="right" valign="top" class="title"><span class="rank">24.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38175213'href='vote?id=38175213&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://cowboyprogramming.com/2007/01/05/evolve-your-heirachy/" rel="noreferrer">Evolve Your Hierarchy (2007)</a><span class="sitebit comhead"> (<a href="from?site=cowboyprogramming.com"><span class="sitestr">cowboyprogramming.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38175213">13 points</span> by <a href="user?id=lambda_garden" class="hnuser">lambda_garden</a> <span class="age" title="2023-11-07T10:23:35"><a href="item?id=38175213">4 hours ago</a></span> <span id="unv_38175213"></span> | <a href="hide?id=38175213&amp;goto=news">hide</a> | <a href="item?id=38175213">1&nbsp;comment</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38199670'>
      <td align="right" valign="top" class="title"><span class="rank">25.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38199670'href='vote?id=38199670&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.cnn.com/2023/11/08/tech/meta-facebook-instagram-teen-safety/index.html" rel="noreferrer">Zuckerberg personally rejected Meta&#x27;s proposals to improve teen mental health</a><span class="sitebit comhead"> (<a href="from?site=cnn.com"><span class="sitestr">cnn.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38199670">16 points</span> by <a href="user?id=miguelazo" class="hnuser">miguelazo</a> <span class="age" title="2023-11-09T01:17:02"><a href="item?id=38199670">26 minutes ago</a></span> <span id="unv_38199670"></span> | <a href="hide?id=38199670&amp;goto=news">hide</a> | <a href="item?id=38199670">discuss</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38188788'>
      <td align="right" valign="top" class="title"><span class="rank">26.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38188788'href='vote?id=38188788&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://opusmodus.com/" rel="noreferrer">Opusmodus: Common Lisp Music Composition System</a><span class="sitebit comhead"> (<a href="from?site=opusmodus.com"><span class="sitestr">opusmodus.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38188788">154 points</span> by <a href="user?id=zetalyrae" class="hnuser">zetalyrae</a> <span class="age" title="2023-11-08T10:57:11"><a href="item?id=38188788">14 hours ago</a></span> <span id="unv_38188788"></span> | <a href="hide?id=38188788&amp;goto=news">hide</a> | <a href="item?id=38188788">53&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38182197'>
      <td align="right" valign="top" class="title"><span class="rank">27.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38182197'href='vote?id=38182197&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://aeon.co/essays/since-when-is-philosophy-a-branch-of-the-self-help-industry" rel="noreferrer">Whither philosophy?</a><span class="sitebit comhead"> (<a href="from?site=aeon.co"><span class="sitestr">aeon.co</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38182197">34 points</span> by <a href="user?id=apollinaire" class="hnuser">apollinaire</a> <span class="age" title="2023-11-07T20:17:53"><a href="item?id=38182197">9 hours ago</a></span> <span id="unv_38182197"></span> | <a href="hide?id=38182197&amp;goto=news">hide</a> | <a href="item?id=38182197">70&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38193494'>
      <td align="right" valign="top" class="title"><span class="rank">28.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38193494'href='vote?id=38193494&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.bbc.com/news/uk-england-wiltshire-67336495" rel="noreferrer">Original photo from Led Zeppelin IV album cover discovered</a><span class="sitebit comhead"> (<a href="from?site=bbc.com"><span class="sitestr">bbc.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38193494">102 points</span> by <a href="user?id=boulos" class="hnuser">boulos</a> <span class="age" title="2023-11-08T17:11:31"><a href="item?id=38193494">8 hours ago</a></span> <span id="unv_38193494"></span> | <a href="hide?id=38193494&amp;goto=news">hide</a> | <a href="item?id=38193494">30&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38194359'>
      <td align="right" valign="top" class="title"><span class="rank">29.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38194359'href='vote?id=38194359&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.theguardian.com/us-news/2023/nov/08/texas-methane-oil-and-gas-study-climate" rel="noreferrer">Oil and gas production in Texas produces twice as much methane as in New Mexico</a><span class="sitebit comhead"> (<a href="from?site=theguardian.com"><span class="sitestr">theguardian.com</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38194359">212 points</span> by <a href="user?id=webmaven" class="hnuser">webmaven</a> <span class="age" title="2023-11-08T18:04:23"><a href="item?id=38194359">7 hours ago</a></span> <span id="unv_38194359"></span> | <a href="hide?id=38194359&amp;goto=news">hide</a> | <a href="item?id=38194359">86&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
                <tr class='athing' id='38195718'>
      <td align="right" valign="top" class="title"><span class="rank">30.</span></td>      <td valign="top" class="votelinks"><center><a id='up_38195718'href='vote?id=38195718&amp;how=up&amp;goto=news'><div class='votearrow' title='upvote'></div></a></center></td><td class="title"><span class="titleline"><a href="https://www.seti.org/press-release/200m-gift-propels-scientific-research-search-life-beyond-earth" rel="noreferrer">$200M gift propels scientific research in the search for life beyond Earth</a><span class="sitebit comhead"> (<a href="from?site=seti.org"><span class="sitestr">seti.org</span></a>)</span></span></td></tr><tr><td colspan="2"></td><td class="subtext"><span class="subline">
          <span class="score" id="score_38195718">124 points</span> by <a href="user?id=webmaven" class="hnuser">webmaven</a> <span class="age" title="2023-11-08T19:32:15"><a href="item?id=38195718">6 hours ago</a></span> <span id="unv_38195718"></span> | <a href="hide?id=38195718&amp;goto=news">hide</a> | <a href="item?id=38195718">84&nbsp;comments</a>        </span>
              </td></tr>
      <tr class="spacer" style="height:5px"></tr>
            <tr class="morespace" style="height:10px"></tr><tr><td colspan="2"></td>
      <td class='title'><a href='?p=2' class='morelink' rel='next'>More</a></td>    </tr>
  </table>
</td></tr>
<tr><td><img src="s.gif" height="10" width="0"><table width="100%" cellspacing="0" cellpadding="1"><tr><td bgcolor="#ff6600"></td></tr></table><br>
<center><span class="yclinks"><a href="newsguidelines.html">Guidelines</a> | <a href="newsfaq.html">FAQ</a> | <a href="lists">Lists</a> | <a href="https://github.com/HackerNews/API">API</a> | <a href="security.html">Security</a> | <a href="https://www.ycombinator.com/legal/">Legal</a> | <a href="https://www.ycombinator.com/apply/">Apply to YC</a> | <a href="mailto:hn@ycombinator.com">Contact</a></span><br><br>
<form method="get" action="//hn.algolia.com/">Search: <input type="text" name="q" size="17" autocorrect="off" spellcheck="false" autocapitalize="off" autocomplete="false"></form></center></td></tr>      </table></center></body>
      <script type='text/javascript' src='hn.js?lkyh8o6g5AgZZ2PaGgEr'></script>
  </html>"###.to_string()
    }
}
