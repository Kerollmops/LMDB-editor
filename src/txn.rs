use std::mem;

use heed::Env;

use heed::RwTxn;

use heed::RoTxn;

pub(crate) enum Txn {
    /// A read-only transaction.
    Ro(RoTxn<'static>),
    /// A read-write transaction.
    Rw(RwTxn<'static>),
    None,
}

impl Txn {
    /// Commit read-write transaction and change it to read-only. Noop for `Txn::Ro`.
    pub(crate) fn commit(&mut self, env: &'static Env) {
        self.end_rw(env, |wtxn| wtxn.commit().unwrap());
    }

    /// Abort read-write transaction and change it to read-only. Noop for `Txn::Ro`.
    pub(crate) fn abort(&mut self, env: &'static Env) {
        self.end_rw(env, |wtxn| wtxn.abort());
    }

    /// Refresh the current read transaction. Noop fro `Txn::Rw`.
    pub(crate) fn refresh(&mut self, env: &'static Env) {
        if matches!(self, Self::Ro(_)) {
            // We must drop the rtxn before opening a new one as it is forbidden
            // to have two transactions on the same thread at any given time.
            let rtxn = mem::replace(self, Self::None);
            drop(rtxn);
            *self = Self::Ro(env.read_txn().unwrap());
        }
    }

    pub(crate) fn end_rw(&mut self, env: &'static Env, f: fn(RwTxn<'static>)) {
        match self {
            Self::Ro(_) => (),
            Self::None => unreachable!(),
            Self::Rw(_) => {
                // We should call `f` (which commits or aborts the read-write
                // transaction) before creating a new read-only transaction,
                // otherwise the read-only transaction will not see the changes
                // made by the read-write transaction.
                match mem::replace(self, Self::None) {
                    Self::Rw(wtxn) => f(wtxn),
                    Self::Ro(_) | Self::None => unreachable!(),
                }
                let rtxn = env.read_txn().unwrap();
                match mem::replace(self, Self::Ro(rtxn)) {
                    Self::None => (),
                    Self::Ro(_) | Self::Rw(_) => unreachable!(),
                }
            }
        }
    }
}
