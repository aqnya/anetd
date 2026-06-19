use log::info;
use std::io;

use crate::handlers::{CommandCtx, CommandHandler};
use crate::server::{ProtoWrite, connect_netd, proxy_transparent};

pub struct SetOperatorAddressHandler;

impl CommandHandler for SetOperatorAddressHandler {
    fn handle(&self, ctx: CommandCtx) -> io::Result<()> {
        let CommandCtx {
            client, cmd_line, ..
        } = ctx;

        info!("[handler] Handling setoperatoraddress command transparently");

        let mut netd = connect_netd()?;

        netd.write_cmd(cmd_line)?;

        proxy_transparent(client, netd)?;

        Ok(())
    }
}
