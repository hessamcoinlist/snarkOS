pub(self) mod connection_handler;

pub(self) mod message_handler;

pub mod miner_instance;
pub use self::miner_instance::*;

pub mod server;
pub use self::server::*;

use crate::{
    message::types::{Block, Transaction},
    Context,
};
use snarkos_consensus::miner::{Entry, MemoryPool as MemoryPoolStruct};
use snarkos_errors::network::SendError;
use snarkos_objects::Transaction as TransactionStruct;
use snarkos_storage::BlockStorage;

use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

/// Process a transaction
pub async fn process_transaction_internal(
    context: Arc<Context>,
    storage: Arc<BlockStorage>,
    memory_pool_lock: Arc<Mutex<MemoryPoolStruct>>,
    transaction_bytes: Vec<u8>,
    transaction_sender: SocketAddr,
) -> Result<(), SendError> {
    if let Ok(transaction) = TransactionStruct::deserialize(&transaction_bytes) {
        let mut memory_pool = memory_pool_lock.lock().await;

        let entry = Entry {
            size: transaction_bytes.len(),
            transaction,
        };

        if let Ok(inserted) = memory_pool.insert(&storage, entry) {
            if inserted.is_some() {
                info!("Transaction added to mempool. Propagating transaction to peers");

                for (socket, _) in &context.peer_book.read().await.peers.addresses {
                    if *socket != transaction_sender && *socket != context.local_addr {
                        if let Some(channel) = context.connections.read().await.get(socket) {
                            channel.write(&Transaction::new(transaction_bytes.clone())).await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Announce block to peers
pub async fn propagate_block(context: Arc<Context>, data: Vec<u8>, block_miner: SocketAddr) -> Result<(), SendError> {
    info!("Propagating block to peers");

    for (socket, _) in &context.peer_book.read().await.peers.addresses {
        if *socket != block_miner && *socket != context.local_addr {
            if let Some(channel) = context.connections.read().await.get(socket) {
                channel.write(&Block::new(data.clone())).await?;
            }
        }
    }
    Ok(())
}