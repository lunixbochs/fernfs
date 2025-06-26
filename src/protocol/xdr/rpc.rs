//! This module provides data structures for the Remote Procedure Call (RPC) protocol
//! as defined in RFC 5531 (previously RFC 1057). These structures handle serialization and deserialization
//! of RPC messages between client and server.

// Allow unused code since we implement the complete RFC specification
#![allow(dead_code)]
// Keep original RFC naming conventions for consistency with the specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::*;

/// Authentication status codes indicating why authentication failed
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum auth_stat {
    /// Invalid credentials provided by client (checksum/signature verification failed)
    #[default]
    AUTH_BADCRED = 1,
    /// Credentials rejected - client needs to establish a new session
    AUTH_REJECTEDCRED = 2,
    /// Invalid verifier provided by client (checksum/signature verification failed)
    AUTH_BADVERF = 3,
    /// Verifier rejected due to expiration or replay attempt
    AUTH_REJECTEDVERF = 4,
    /// Authentication mechanism too weak for requested operation
    AUTH_TOOWEAK = 5,
}
impl SerializeEnum for auth_stat {}
impl DeserializeEnum for auth_stat {}

/// Authentication flavor (mechanism) identifiers for RPC
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
#[non_exhaustive]
pub enum auth_flavor {
    /// No authentication
    AUTH_NULL = 0,
    /// UNIX-style authentication (uid/gid)
    AUTH_UNIX = 1,
    /// Short-form authentication
    AUTH_SHORT = 2,
    /// DES authentication
    AUTH_DES = 3,
    /* and more to be defined */
}
impl SerializeEnum for auth_flavor {}
impl DeserializeEnum for auth_flavor {}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
/// UNIX-style credentials used for authentication
pub struct auth_unix {
    /// Timestamp to prevent replay attacks
    pub stamp: u32,
    /// The name of the client machine
    pub machinename: Vec<u8>,
    /// The effective user ID of the caller
    pub uid: u32,
    /// The effective group ID of the caller
    pub gid: u32,
    /// A list of additional group IDs for the caller
    pub gids: Vec<u32>,
}
DeserializeStruct!(auth_unix, stamp, machinename, uid, gid, gids);
SerializeStruct!(auth_unix, stamp, machinename, uid, gid, gids);

/// Authentication data structure used in RPC protocol for both client and server authentication.
///
/// The RPC protocol provides bidirectional authentication between caller and service:
/// - Call messages contain two auth fields: credentials and verifier
/// - Reply messages contain one auth field: response verifier
///
/// Each auth field is represented as an `opaque_auth` structure containing:
/// - An `auth_flavor` enum identifying the authentication mechanism
/// - Opaque bytes containing the auth data, interpreted based on the mechanism
///
/// The actual authentication data format and validation is defined by the specific
/// authentication protocol being used (e.g. AUTH_UNIX, AUTH_DES etc).
///
/// If authentication fails, the reply message will include details about why the
/// auth parameters were rejected.
///
/// Opaque authentication data structure as defined in RFC 5531 (previously RFC 1057)
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct opaque_auth {
    /// The authentication mechanism being used
    pub flavor: auth_flavor,
    /// The opaque authentication data associated with that mechanism
    pub body: Vec<u8>,
}
DeserializeStruct!(opaque_auth, flavor, body);
SerializeStruct!(opaque_auth, flavor, body);

impl Default for opaque_auth {
    fn default() -> opaque_auth {
        opaque_auth { flavor: auth_flavor::AUTH_NULL, body: Vec::new() }
    }
}

/// RPC message structure as defined in RFC 5531 (previously RFC 1057).
///
/// Each RPC message begins with a transaction identifier (xid) followed by a
/// discriminated union containing either a CALL or REPLY message body.
///
/// The xid serves several purposes:
/// - Clients use it to match REPLY messages with their corresponding CALL messages
/// - Servers use it to detect retransmitted requests
/// - The xid in a REPLY always matches the xid from the initiating CALL
///
/// Note: The xid is not a sequence number and should not be treated as such by servers.
/// It is only used for request/response matching and duplicate detection.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct rpc_msg {
    /// Transaction identifier used to match calls and replies
    pub xid: u32,
    /// The body of the RPC message (call or reply)
    pub body: rpc_body,
}
DeserializeStruct!(rpc_msg, xid, body);
SerializeStruct!(rpc_msg, xid, body);

/// The body of an RPC message, which can be either a call or a reply
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug)]
#[repr(u32)]
pub enum rpc_body {
    /// A call to a remote procedure
    CALL(call_body),
    /// A reply from a remote procedure
    REPLY(reply_body),
}

impl Default for rpc_body {
    fn default() -> rpc_body {
        rpc_body::CALL(call_body::default())
    }
}

impl Serialize for rpc_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            rpc_body::CALL(v) => {
                0_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
            rpc_body::REPLY(v) => {
                1_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}
impl Deserialize for rpc_body {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => *self = rpc_body::CALL(deserialize(src)?),
            1 => *self = rpc_body::REPLY(deserialize(src)?),
            msg_type => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid message type in rpc_body: {msg_type}"),
                ))
            }
        }

        Ok(())
    }
}

/// The body of an RPC call, containing all information needed for a remote procedure call
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct call_body {
    /// RPC version, must be 2
    pub rpcvers: u32,
    /// The program to call
    pub prog: u32,
    /// The version of the program
    pub vers: u32,
    /// The procedure within the program to call
    pub proc: u32,
    /// Authentication credentials for the caller
    pub cred: opaque_auth,
    /// Authentication verifier for the caller
    pub verf: opaque_auth,
    /* procedure specific parameters start here */
}
DeserializeStruct!(call_body, rpcvers, prog, vers, proc, cred, verf);
SerializeStruct!(call_body, rpcvers, prog, vers, proc, cred, verf);

/// The body of an RPC reply, indicating whether the call was accepted or denied
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum reply_body {
    /// The call was accepted
    MSG_ACCEPTED(accepted_reply),
    /// The call was denied
    MSG_DENIED(rejected_reply),
}

impl Default for reply_body {
    fn default() -> reply_body {
        reply_body::MSG_ACCEPTED(accepted_reply::default())
    }
}

impl Serialize for reply_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            reply_body::MSG_ACCEPTED(v) => {
                0_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
            reply_body::MSG_DENIED(v) => {
                1_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }
        Ok(())
    }
}
impl Deserialize for reply_body {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => *self = reply_body::MSG_ACCEPTED(deserialize(src)?),
            1 => *self = reply_body::MSG_DENIED(deserialize(src)?),
            reply_status => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid reply status in reply_body: {reply_status}"),
                ))
            }
        }

        Ok(())
    }
}

/// Information about program version mismatch
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct mismatch_info {
    /// Lowest version supported
    pub low: u32,
    /// Highest version supported
    pub high: u32,
}
DeserializeStruct!(mismatch_info, low, high);
SerializeStruct!(mismatch_info, low, high);

/// Reply to an RPC call that was accepted by the server.
///
/// Even though the call was accepted, there could still be an error in processing it.
/// The structure contains:
/// - An authentication verifier generated by the server to validate itself to the client
/// - A union containing the actual reply data, discriminated by accept_stat enum
///
/// The reply_data union has the following arms:
/// - SUCCESS: Contains protocol-specific success response
/// - PROG_UNAVAIL: Program not available (void)
/// - PROG_MISMATCH: Program version mismatch, includes supported version range
/// - PROC_UNAVAIL: Procedure not available (void)
/// - GARBAGE_ARGS: Arguments could not be decoded (void)
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct accepted_reply {
    /// Authentication verifier from server
    pub verf: opaque_auth,
    /// Reply data union discriminated by accept_stat
    pub reply_data: accept_body,
}
DeserializeStruct!(accepted_reply, verf, reply_data);
SerializeStruct!(accepted_reply, verf, reply_data);

/// Response data for an accepted RPC call, discriminated by accept_stat.
///
/// This enum represents the possible outcomes of an accepted RPC call:
/// - SUCCESS: Call completed successfully, response data is protocol-specific
/// - PROG_UNAVAIL: The requested program is not available on this server
/// - PROG_MISMATCH: Program version mismatch, includes supported version range
/// - PROC_UNAVAIL: The requested procedure is not available in this program
/// - GARBAGE_ARGS: The server could not decode the call arguments
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Default)]
#[repr(u32)]
pub enum accept_body {
    /// Call completed successfully
    #[default]
    SUCCESS,
    /// Program is not available on this server
    PROG_UNAVAIL,
    /// Program version mismatch, includes supported version range
    PROG_MISMATCH(mismatch_info),
    /// Requested procedure is not available
    PROC_UNAVAIL,
    /// Server could not decode the call arguments
    GARBAGE_ARGS,
}

impl Serialize for accept_body {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            accept_body::SUCCESS => {
                0_u32.serialize(dest)?;
            }
            accept_body::PROG_UNAVAIL => {
                1_u32.serialize(dest)?;
            }
            accept_body::PROG_MISMATCH(v) => {
                2_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
            accept_body::PROC_UNAVAIL => {
                3_u32.serialize(dest)?;
            }
            accept_body::GARBAGE_ARGS => {
                4_u32.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for accept_body {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => *self = accept_body::SUCCESS,
            1 => *self = accept_body::PROG_UNAVAIL,
            2 => *self = accept_body::PROG_MISMATCH(deserialize(src)?),
            3 => *self = accept_body::PROC_UNAVAIL,
            4 => *self = accept_body::GARBAGE_ARGS,
            accept_stat => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid accept stat in accept_body: {accept_stat}"),
                ));
            }
        }

        Ok(())
    }
}

/// Reply sent when an RPC call is rejected by the server.
///
/// The call can be rejected for two reasons:
/// 1. RPC Version Mismatch (RPC_MISMATCH):
///    - Server is not running a compatible version of the RPC protocol
///    - Server returns the lowest and highest supported RPC versions
///
/// 2. Authentication Error (AUTH_ERROR):
///    - Server refuses to authenticate the caller
///    - Returns specific auth failure status code
///
/// The discriminant for this enum is `reject_stat` which indicates the
/// rejection reason.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum rejected_reply {
    /// RPC version mismatch - includes supported version range
    RPC_MISMATCH(mismatch_info),
    /// Authentication failed - includes specific error code
    AUTH_ERROR(auth_stat),
}

impl Default for rejected_reply {
    fn default() -> rejected_reply {
        rejected_reply::AUTH_ERROR(auth_stat::default())
    }
}

impl Serialize for rejected_reply {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            rejected_reply::RPC_MISMATCH(v) => {
                0_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
            rejected_reply::AUTH_ERROR(v) => {
                1_u32.serialize(dest)?;
                (*v as u32).serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for rejected_reply {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match deserialize::<u32>(src)? {
            0 => *self = rejected_reply::RPC_MISMATCH(deserialize(src)?),
            1 => *self = rejected_reply::AUTH_ERROR(deserialize(src)?),
            stat => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid reject stat in rejected_reply: {stat}"),
                ))
            }
        }

        Ok(())
    }
}

/// Creates a reply message indicating that the requested procedure is not available
pub fn proc_unavail_reply_message(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::PROC_UNAVAIL,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a reply message indicating that the requested program is not available
pub fn prog_unavail_reply_message(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::PROG_UNAVAIL,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a reply message indicating a program version mismatch
pub fn prog_mismatch_reply_message(xid: u32, accepted_ver: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::PROG_MISMATCH(mismatch_info {
            low: accepted_ver,
            high: accepted_ver,
        }),
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a reply message indicating that the arguments could not be decoded
pub fn garbage_args_reply_message(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::GARBAGE_ARGS,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a reply message indicating an RPC version mismatch
pub fn rpc_vers_mismatch(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_DENIED(rejected_reply::RPC_MISMATCH(mismatch_info::default()));
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}

/// Creates a successful reply message with no additional data
pub fn make_success_reply(xid: u32) -> rpc_msg {
    let reply = reply_body::MSG_ACCEPTED(accepted_reply {
        verf: opaque_auth::default(),
        reply_data: accept_body::SUCCESS,
    });
    rpc_msg { xid, body: rpc_body::REPLY(reply) }
}
