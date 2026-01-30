use std::collections::HashMap;

use crate::account::Account;
use crate::transaction::{TransactionRecord, TransactionType};

#[derive(Debug, Clone)]
struct StoredDeposit {
    client: u16,
    amount: f64,
    disputed: bool,
}

#[derive(Default)]
pub struct PaymentEngine {
    accounts: HashMap<u16, Account>,
    deposits: HashMap<u32, StoredDeposit>,
}

impl PaymentEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process(&mut self, tx: &TransactionRecord) {
        if let Some(account) = self.accounts.get(&tx.client)
            && account.locked
        {
            return;
        }

        match tx.tx_type {
            TransactionType::Deposit => self.deposit(tx),
            TransactionType::Withdrawal => self.withdrawal(tx),
            TransactionType::Dispute => self.dispute(tx),
            TransactionType::Resolve => self.resolve(tx),
            TransactionType::Chargeback => self.chargeback(tx),
        }
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    fn deposit(&mut self, tx: &TransactionRecord) {
        let amount = tx.amount.unwrap();
        if amount <= 0.0 {
            return;
        }

        if self.deposits.contains_key(&tx.tx) {
            return;
        }

        let account = self
            .accounts
            .entry(tx.client)
            .or_insert_with(|| Account::new(tx.client));
        account.available += amount;

        self.deposits.insert(
            tx.tx,
            StoredDeposit {
                client: tx.client,
                amount,
                disputed: false,
            },
        );
    }

    fn withdrawal(&mut self, tx: &TransactionRecord) {
        let amount = tx.amount.unwrap();
        if amount <= 0.0 {
            return;
        }

        let account = self
            .accounts
            .entry(tx.client)
            .or_insert_with(|| Account::new(tx.client));
        if account.available < amount {
            return;
        }

        account.available -= amount;
    }

    fn dispute(&mut self, tx: &TransactionRecord) {
        let deposit = match self.deposits.get_mut(&tx.tx) {
            Some(d) => d,
            None => return,
        };

        if deposit.disputed || deposit.client != tx.client {
            return;
        }

        deposit.disputed = true;
        let account = self.accounts.get_mut(&tx.client).unwrap();
        account.available -= deposit.amount;
        account.held += deposit.amount;
    }

    fn resolve(&mut self, tx: &TransactionRecord) {
        let deposit = match self.deposits.get_mut(&tx.tx) {
            Some(d) => d,
            None => return,
        };

        if !deposit.disputed || deposit.client != tx.client {
            return;
        }

        deposit.disputed = false;
        let account = self.accounts.get_mut(&tx.client).unwrap();
        account.held -= deposit.amount;
        account.available += deposit.amount;
    }

    fn chargeback(&mut self, tx: &TransactionRecord) {
        let deposit = match self.deposits.get_mut(&tx.tx) {
            Some(d) => d,
            None => return,
        };

        if !deposit.disputed || deposit.client != tx.client {
            return;
        }

        deposit.disputed = false;
        let account = self.accounts.get_mut(&tx.client).unwrap();
        account.held -= deposit.amount;
        account.locked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{TransactionRecord, TransactionType};

    fn tx(
        tx_type: TransactionType,
        client: u16,
        tx: u32,
        amount: Option<f64>,
    ) -> TransactionRecord {
        TransactionRecord {
            tx_type,
            client,
            tx,
            amount,
        }
    }

    fn get_account(engine: &PaymentEngine, client: u16) -> &Account {
        engine.accounts().find(|a| a.client == client).unwrap()
    }

    #[test]
    fn deposit_increases_available() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
        assert!((account.total() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Deposit, 1, 2, Some(5.0)));

        let account = get_account(&engine, 1);
        assert!((account.available - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn duplicate_deposit_tx_id_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(99.0)));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn withdrawal_decreases_available() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(4.0)));

        let account = get_account(&engine, 1);
        assert!((account.available - 6.0).abs() < f64::EPSILON);
        assert!((account.total() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn withdrawal_insufficient_funds_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(5.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(10.0)));

        let account = get_account(&engine, 1);
        assert!((account.available - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn withdrawal_exact_balance() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(7.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(7.0)));

        let account = get_account(&engine, 1);
        assert!((account.available).abs() < f64::EPSILON);
    }

    #[test]
    fn dispute_moves_to_held() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.available).abs() < f64::EPSILON);
        assert!((account.held - 10.0).abs() < f64::EPSILON);
        assert!((account.total() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dispute_nonexistent_tx_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 999, None));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
    }

    #[test]
    fn dispute_already_disputed_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.held - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dispute_wrong_client_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Deposit, 2, 2, Some(5.0)));
        engine.process(&tx(TransactionType::Dispute, 2, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_moves_back_to_available() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        engine.process(&tx(TransactionType::Resolve, 1, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
        assert!((account.total() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_not_disputed_ignored() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Resolve, 1, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.available - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn chargeback_removes_held_and_locks() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        engine.process(&tx(TransactionType::Chargeback, 1, 1, None));

        let account = get_account(&engine, 1);
        assert!((account.available).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
        assert!((account.total()).abs() < f64::EPSILON);
        assert!(account.locked);
    }

    #[test]
    fn frozen_account_rejects_all() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        engine.process(&tx(TransactionType::Chargeback, 1, 1, None));

        engine.process(&tx(TransactionType::Deposit, 1, 2, Some(50.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(1.0)));

        let account = get_account(&engine, 1);
        assert!(account.locked);
        assert!((account.available).abs() < f64::EPSILON);
        assert!((account.total()).abs() < f64::EPSILON);
    }

    #[test]
    fn multiple_clients_independent() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(10.0)));
        engine.process(&tx(TransactionType::Deposit, 2, 2, Some(20.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(5.0)));

        let a1 = get_account(&engine, 1);
        let a2 = get_account(&engine, 2);
        assert!((a1.available - 5.0).abs() < f64::EPSILON);
        assert!((a2.available - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn full_dispute_resolve_lifecycle() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(100.0)));
        engine.process(&tx(TransactionType::Deposit, 1, 2, Some(50.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(30.0)));

        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        let account = get_account(&engine, 1);
        assert!((account.available - 20.0).abs() < f64::EPSILON);
        assert!((account.held - 100.0).abs() < f64::EPSILON);
        assert!((account.total() - 120.0).abs() < f64::EPSILON);

        engine.process(&tx(TransactionType::Resolve, 1, 1, None));
        let account = get_account(&engine, 1);
        assert!((account.available - 120.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
        assert!(!account.locked);
    }

    #[test]
    fn full_dispute_chargeback_lifecycle() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(100.0)));
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(40.0)));

        engine.process(&tx(TransactionType::Dispute, 1, 1, None));
        let account = get_account(&engine, 1);
        assert!((account.available - -40.0).abs() < f64::EPSILON);
        assert!((account.held - 100.0).abs() < f64::EPSILON);

        engine.process(&tx(TransactionType::Chargeback, 1, 1, None));
        let account = get_account(&engine, 1);
        assert!((account.available - -40.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
        assert!((account.total() - -40.0).abs() < f64::EPSILON);
        assert!(account.locked);
    }
}
