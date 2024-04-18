use crate::{
    cli::{arguments, node::Node},
    config::NodeConfig,
};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    filter::Targets,
    fmt::format::{DefaultVisitor, Writer},
    layer::Layer,
    prelude::*,
    util::SubscriberInitExt,
};

pub async fn run(args: arguments::Run) -> eyre::Result<()> {
    let config = NodeConfig::from_path(args.config)?;

    let level_filter = config.logger.level;

    let filter = Targets::new()
        .with_target("yuv_indexers", level_filter)
        .with_target("yuv_controller", level_filter)
        .with_target("yuv_rpc_server", level_filter)
        .with_target("yuv_network", level_filter)
        .with_target("yuv_tx_attach", level_filter)
        .with_target("yuv_tx_check", level_filter)
        .with_target("yuv_p2p", level_filter)
        .with_default(level_filter);

    tracing_subscriber::registry()
        .with(YuvTracer.with_filter(filter))
        .try_init()?;

    // Start all main components, but do not start external services
    // like RPC, p2p until indexer will be initialized.

    let node = Node::new(config).await?;
    node.run().await?;

    tokio::signal::ctrl_c().await?;

    node.shutdown().await;

    Ok(())
}

struct YuvTracer;

impl<S> Layer<S> for YuvTracer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let target = match event.metadata().level() {
            &Level::INFO | &Level::WARN | &Level::ERROR => event
                .metadata()
                .target()
                .split("::")
                .last()
                .unwrap_or_default(),
            _ => event.metadata().target(),
        };

        print!(
            "[{}] {} {}: ",
            chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S"),
            event.metadata().level(),
            target,
        );

        let mut message = String::new();

        event.record(&mut DefaultVisitor::new(Writer::new(&mut message), true));

        println!("{}", message);
    }
}
