use crate::protocol::xdr::{self, nfs3};

use super::Capabilities;

#[derive(Clone, Copy, Debug)]
pub struct UnixPerms {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

fn auth_matches_gid(auth: &xdr::rpc::auth_unix, gid: nfs3::gid3) -> bool {
    auth.gid == gid || auth.gids.contains(&gid)
}

pub fn unix_mode_perms(attr: &nfs3::fattr3, auth: &xdr::rpc::auth_unix) -> UnixPerms {
    if auth.uid == 0 {
        return UnixPerms { read: true, write: true, exec: true };
    }

    let mode = attr.mode & 0o777;
    if auth.uid == attr.uid {
        UnixPerms { read: mode & 0o400 != 0, write: mode & 0o200 != 0, exec: mode & 0o100 != 0 }
    } else if auth_matches_gid(auth, attr.gid) {
        UnixPerms { read: mode & 0o040 != 0, write: mode & 0o020 != 0, exec: mode & 0o010 != 0 }
    } else {
        UnixPerms { read: mode & 0o004 != 0, write: mode & 0o002 != 0, exec: mode & 0o001 != 0 }
    }
}

pub fn access_mask(
    attr: &nfs3::fattr3,
    auth: &xdr::rpc::auth_unix,
    capabilities: Capabilities,
    requested: u32,
) -> u32 {
    let perms = unix_mode_perms(attr, auth);
    let supports_write = matches!(capabilities, Capabilities::ReadWrite);
    let allow_write = supports_write && perms.write;
    let write_mask = nfs3::ACCESS3_MODIFY | nfs3::ACCESS3_EXTEND;
    let delete_mask = nfs3::ACCESS3_DELETE;
    let mut granted_access = 0;

    match attr.ftype {
        nfs3::ftype3::NF3REG => {
            if requested & nfs3::ACCESS3_READ != 0 && perms.read {
                granted_access |= nfs3::ACCESS3_READ;
            }
            if requested & nfs3::ACCESS3_EXECUTE != 0 && perms.exec {
                granted_access |= nfs3::ACCESS3_EXECUTE;
            }
            if requested & write_mask != 0 && allow_write {
                granted_access |= requested & write_mask;
            }
        }
        nfs3::ftype3::NF3DIR => {
            if requested & nfs3::ACCESS3_READ != 0 && perms.read {
                granted_access |= nfs3::ACCESS3_READ;
            }
            if requested & nfs3::ACCESS3_LOOKUP != 0 && perms.exec {
                granted_access |= nfs3::ACCESS3_LOOKUP;
            }
            if requested & nfs3::ACCESS3_EXECUTE != 0 && perms.exec {
                granted_access |= nfs3::ACCESS3_EXECUTE;
            }
            if requested & (write_mask | delete_mask) != 0 && allow_write && perms.exec {
                granted_access |= requested & (write_mask | delete_mask);
            }
        }
        nfs3::ftype3::NF3LNK => {
            if requested & nfs3::ACCESS3_READ != 0 && perms.read {
                granted_access |= nfs3::ACCESS3_READ;
            }
            if requested & nfs3::ACCESS3_EXECUTE != 0 && perms.exec {
                granted_access |= nfs3::ACCESS3_EXECUTE;
            }
        }
        _ => {
            if requested & nfs3::ACCESS3_READ != 0 && perms.read {
                granted_access |= nfs3::ACCESS3_READ;
            }
            if requested & nfs3::ACCESS3_EXECUTE != 0 && perms.exec {
                granted_access |= nfs3::ACCESS3_EXECUTE;
            }
        }
    }

    granted_access
}
