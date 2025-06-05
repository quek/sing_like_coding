use tokio::net::windows::named_pipe::NamedPipeServer;

pub struct PluginRef {
    pub pipe: NamedPipeServer,
}

impl PluginRef {
    pub fn new(pipe: NamedPipeServer) -> Self {
        Self { pipe }
    }
}
