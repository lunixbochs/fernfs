//! Command queue for ordered processing of RPC commands
//!
//! This module provides a command queue system that ensures RPC operations
//! are processed in the exact order they were received, preserving FIFO semantics
//! necessary for proper NFS protocol operation.

use anyhow::anyhow;
use tokio::sync::mpsc;
use tracing::{debug, error, trace};

use crate::protocol::rpc;

/// Represents a response buffer that minimizes data copying
pub struct ResponseBuffer {
    /// Internal buffer for writing data
    buffer: Vec<u8>,
    /// Indicates that the buffer contains data to send
    has_content: bool,
}

impl ResponseBuffer {
    /// Creates a new response buffer with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self { buffer: Vec::with_capacity(capacity), has_content: false }
    }

    /// Gets the internal buffer for writing
    pub fn get_mut_buffer(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Marks the buffer as containing data to send
    pub fn mark_has_content(&mut self) {
        self.has_content = true;
    }

    /// Checks if the buffer contains data to send
    pub fn has_content(&self) -> bool {
        self.has_content
    }

    /// Takes the internal buffer, consuming the structure
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    /// Clears the buffer for reuse
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.has_content = false;
    }
}

/// RPC command type with context
#[derive(Debug)]
pub struct RpcCommand {
    /// RPC message data
    pub data: Vec<u8>,
    /// Context associated with this command
    pub context: rpc::Context,
}

/// Command processing result
pub type CommandResult = Result<Option<ResponseBuffer>, anyhow::Error>;

/// Type for asynchronous RPC command processor
pub type AsyncCommandProcessor = for<'a> fn(
    data: &[u8],
    output: &'a mut ResponseBuffer,
    context: rpc::Context,
)
    -> futures::future::BoxFuture<'a, anyhow::Result<bool>>;

/// Queue for sequential processing of RPC commands
///
/// This structure manages an unbounded queue of RPC commands and processes
/// them sequentially to ensure proper operation order:
///
/// - Guaranteed FIFO command processing
/// - Asynchronous command submission
/// - Minimized data copying
/// - Separation of command submission from processing
#[derive(Debug, Clone)]
pub struct CommandQueue {
    /// Channel for sending commands
    command_sender: mpsc::UnboundedSender<RpcCommand>,
}

impl CommandQueue {
    /// Creates a new command queue with the given processor
    ///
    /// Initializes the command queue and starts a worker task that will
    /// process submitted commands in order. The processor function is
    /// responsible for handling each command and creating the result.
    ///
    /// # Arguments
    ///
    /// * `processor` - Asynchronous function for processing RPC commands
    /// * `result_sender` - Channel for sending processing results
    /// * `buffer_capacity` - Initial capacity for response buffers
    pub fn new(
        processor: AsyncCommandProcessor,
        result_sender: mpsc::UnboundedSender<CommandResult>,
        buffer_capacity: usize,
    ) -> Self {
        let (command_sender, mut command_receiver) = mpsc::unbounded_channel::<RpcCommand>();

        // Start worker task that processes commands in order
        tokio::spawn(async move {
            // Create reusable buffer for responses
            let mut output_buffer = ResponseBuffer::with_capacity(buffer_capacity);

            while let Some(command) = command_receiver.recv().await {
                trace!("Processing command from queue");

                // Clear buffer for reuse
                output_buffer.clear();

                // Call async processor
                let result =
                    match processor(&command.data, &mut output_buffer, command.context).await {
                        Ok(true) => {
                            // Processor indicated response needs to be sent
                            output_buffer.mark_has_content();
                            let buffer_to_send = std::mem::replace(
                                &mut output_buffer,
                                ResponseBuffer::with_capacity(buffer_capacity),
                            );
                            Ok(Some(buffer_to_send))
                        }
                        Ok(false) => {
                            // No response needed (e.g. retransmission)
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    };

                // Send result
                if let Err(e) = result_sender.send(result) {
                    error!("Failed to send command processing result: {:?}", e);
                    break;
                }
            }
            debug!("Command queue handler finished");
        });

        Self { command_sender }
    }

    /// Submits a command to the queue for processing
    ///
    /// Commands are processed in the order they are submitted.
    /// This is an asynchronous operation that returns control immediately.
    ///
    /// # Arguments
    ///
    /// * `data` - RPC message data
    /// * `context` - Context for processing this command
    ///
    /// # Returns
    ///
    /// `Ok(())` if command was successfully submitted,
    /// `Err` if submission failed (e.g. if queue was closed)
    pub fn submit_command(
        &self,
        data: Vec<u8>,
        context: rpc::Context,
    ) -> Result<(), anyhow::Error> {
        self.command_sender
            .send(RpcCommand { data, context })
            .map_err(|e| anyhow!("Failed to send command: {}", e))
    }
}
