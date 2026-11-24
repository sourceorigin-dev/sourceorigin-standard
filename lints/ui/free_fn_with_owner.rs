//! `free_fn_with_owner`'s cases: a free function whose first parameter is a
//! type of this crate is flagged; methods, `main`, tests and functions
//! taking foreign types or axum extractors first are not.

use axum_core::extract::FromRequestParts;
use http::request::Parts;

struct Account {
    balance: u64,
}

enum Plan {
    Free,
    Paid,
}

fn by_value(account: Account) -> u64 {
    account.balance
}

fn by_ref(account: &Account, extra: u64) -> u64 {
    account.balance + extra
}

fn by_mut(account: &mut Account) {
    account.balance += 1;
}

fn boxed(plan: Box<Plan>) -> bool {
    matches!(*plan, Plan::Paid)
}

fn foreign_first(name: &str, account: &Account) -> usize {
    name.len() + by_ref(account, 0) as usize
}

struct Session;

impl<S: Sync> FromRequestParts<S> for Session {
    type Rejection = ();

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, ()> {
        Ok(Self)
    }
}

async fn handler(_session: Session, account: Account) -> u64 {
    account.balance
}

fn no_parameters() -> Plan {
    Plan::Free
}

impl Account {
    fn method(&self) -> u64 {
        self.balance
    }

    fn associated(account: &Self) -> u64 {
        account.balance
    }
}

#[cfg(test)]
mod tests {
    fn helper(account: &super::Account) -> u64 {
        account.balance
    }
}

#[expect(free_fn_with_owner, reason = "the case an exception covers")]
fn expected(account: &Account) -> u64 {
    account.balance
}

fn main() {}
