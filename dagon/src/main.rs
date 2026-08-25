//! Dagon 命令行入口。

use clap::{Parser, Subcommand};

use dagon::commands::{
    Ctx, cmd_add, cmd_build, cmd_clean, cmd_init, cmd_new, cmd_publish, cmd_remove, cmd_run,
    cmd_search, cmd_test, cmd_update,
};

#[derive(Parser)]
#[command(
    name = "dagon",
    version,
    about = "Rlyeh 包管理器（MVP）",
    long_about = "Dagon 是 Rlyeh 语言的包管理器：项目脚手架、依赖解析、注册表发布与构建集成。\n\n示例:\n  dagon new myapp && cd myapp\n  dagon add foo@^1.0\n  dagon build && dagon run\n  dagon publish --registry /tmp/registry\n  dagon search foo"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 详细输出
    #[arg(long, global = true)]
    verbose: bool,

    /// 注册表地址（默认 ~/.rl/registry；支持路径或 http://）
    #[arg(long, global = true, value_name = "URL")]
    registry: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// 创建新项目
    New {
        /// 项目名
        name: String,
        /// 创建库项目（src/lib.rl）
        #[arg(long)]
        lib: bool,
    },
    /// 初始化当前目录为项目
    Init {
        /// 创建库项目
        #[arg(long)]
        lib: bool,
    },
    /// 添加依赖（name[@req]，如 foo@^1.0）
    Add {
        package: String,
        /// 加入 dev-dependencies
        #[arg(long)]
        dev: bool,
    },
    /// 移除依赖
    Remove {
        package: String,
        /// 从 dev-dependencies 移除
        #[arg(long)]
        dev: bool,
    },
    /// 编译项目
    Build {
        /// release 构建
        #[arg(long)]
        release: bool,
    },
    /// 编译并运行（程序参数需以 -- 分隔：dagon run -- --flag x）
    Run {
        /// 传给程序的参数
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// 构建并运行测试（tests/*.rl）
    Test,
    /// 重新解析依赖并更新 Rlyeh.lock
    Update {
        /// 指定包名（MVP 全量更新，忽略此参数）
        package: Option<String>,
    },
    /// 打包并发布到注册表
    Publish,
    /// 在注册表中搜索包
    Search {
        query: String,
    },
    /// 清理 target 目录
    Clean,
}

fn main() {
    let cli = Cli::parse();
    let mut ctx = Ctx::new(cli.verbose);
    ctx.registry = cli.registry.clone();

    let result = match cli.command {
        Commands::New { name, lib } => cmd_new(&ctx, &name, lib),
        Commands::Init { lib } => cmd_init(&ctx, lib),
        Commands::Add { package, dev } => cmd_add(&ctx, &package, dev),
        Commands::Remove { package, dev } => cmd_remove(&ctx, &package, dev),
        Commands::Build { release } => cmd_build(&ctx, release),
        Commands::Run { args } => cmd_run(&ctx, &args),
        Commands::Test => cmd_test(&ctx),
        Commands::Update { package } => cmd_update(&ctx, package),
        Commands::Publish => cmd_publish(&ctx, None),
        Commands::Search { query } => cmd_search(&ctx, &query, None),
        Commands::Clean => cmd_clean(&ctx),
    };

    if let Err(e) = result {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}
