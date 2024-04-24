### Liquid Cooling Desktop (LCD) Core Library
Rust implementation of miner controller, support Ant/Avalon/Bluestar, more miners support will be added.

This is a core library, you need to build a cli or others binary to use.

Support use feishu sheet as manager UI, send warning msg through feishu chat bot.

```shell
# single lib build
git clone https://github.com/pfcoder/lcd-core.git
cd lcd-core
cargo build --release
```

Library usage: (cron task which perform batch account auto switch)
```rust
use lcd_core::MinersLibConfig;
use log::{info, LevelFilter};
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::fs;
use tokio::select;
use tokio::signal;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

const HOME_DIR: &str = ".miner-cli/";

fn create_home_dir() -> std::io::Result<String> {
    if let Some(mut path) = dirs::home_dir() {
        path.push(HOME_DIR);
        fs::create_dir_all(path.clone())?;
        return Ok(path.to_str().unwrap().to_owned());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Home directory not found",
    ))
}

fn init_log(app_path: &str) {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[Console] {d} - {l} -{t} - {m}{n}",
        )))
        .build();

    // Create a file appender with dynamic log path
    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[File] {d} - {l} - {t} - {m}{n}",
        )))
        .build(app_path.to_owned() + "/log/info.log")
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )
        .unwrap();

    // Use this config
    log4rs::init_config(config).unwrap();
}

fn init_miner(app_path: &str) {
    lcd_core::init(&MinersLibConfig {
        app_path: app_path.to_owned(),
        is_need_db: false,
        // config your feishu app id, app secret, bot token
        feishu_app_id: "xxx".to_owned(),
        feishu_app_secret: "xxx".to_owned(),
        feishu_bot: "xxx".to_owned(),
    });
}

#[tokio::main]
async fn main() -> Result<(), JobSchedulerError> {
    // create home dir if not exist
    let app_path = create_home_dir().unwrap();
    init_log(&app_path);
    init_miner(&app_path);
    let mut sched = JobScheduler::new().await?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(100)
        .enable_all()
        .build()
        .unwrap();

    let runtime_handle = runtime.handle().clone();
    sched
        .add(Job::new_async("0 */5 * * * *", move |_uuid, mut _l| {
            let runtime_handle = runtime_handle.clone();
            Box::pin(async move {
                info!("I run async every 5 minutes");
                lcd_core::switch_if_need(
                    runtime_handle,
                    "xxx",
                    vec!["xx", "xx"],
                    "xx",
                    "xx",
                    "xx",
                )
                .await;
            })
        })?)
        .await?;

    // Add code to be run during/after shutdown
    sched.set_shutdown_handler(Box::new(|| {
        Box::pin(async move {
            info!("Shut down done");
        })
    }));

    // Start the scheduler
    sched.start().await?;

    // Wait for Ctrl+C or the jobs to finish
    select! {
        _ = signal::ctrl_c() => {
            info!("Ctrl+C received, shutting down");
            // shutdown the scheduler
            sched.shutdown().await?;
        },
    }

    Ok(())
}
```