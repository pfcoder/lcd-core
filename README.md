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

Library usage: (tauri)
```rust

use lcd_core::{
    config, init,
    miner::entry::{MachineInfo, PoolConfig},
    reboot, scan, watching, MinersLibConfig,
};

lazy_static! {
    static ref RUNTIME: Runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(20)
        .enable_all()
        .build()
        .unwrap();
}

#[tauri::command]
async fn reboot_machines(ips: Vec<String>) -> Result<(), String> {
    reboot(RUNTIME.handle().clone(), ips).await
}

```