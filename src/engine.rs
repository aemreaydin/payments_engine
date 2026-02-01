use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::dec;

use crate::account::Account;
use crate::error::PaymentError;
use crate::transaction::{TransactionRecord, TransactionType};

#[derive(Debug, Clone)]
struct StoredDeposit {
    client: u16,
    amount: Decimal,
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

    pub fn process(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        if let Some(account) = self.accounts.get(&tx.client)
            && account.locked
        {
            return Err(PaymentError::AccountFrozen(tx.client));
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

    fn deposit(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        let amount = tx.amount.ok_or(PaymentError::MissingAmount(tx.tx))?;
        if amount <= dec!(0) {
            return Err(PaymentError::InvalidAmount(tx.tx, amount));
        }

        if self.deposits.contains_key(&tx.tx) {
            return Err(PaymentError::DuplicateTransaction(tx.tx));
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

        Ok(())
    }

    fn withdrawal(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        let amount = tx.amount.ok_or(PaymentError::MissingAmount(tx.tx))?;
        if amount <= dec!(0) {
            return Err(PaymentError::InvalidAmount(tx.tx, amount));
        }

        let account = self
            .accounts
            .entry(tx.client)
            .or_insert_with(|| Account::new(tx.client));
        if account.available < amount {
            return Err(PaymentError::InsufficientFunds(
                tx.client,
                amount,
                account.available,
            ));
        }

        account.available -= amount;
        Ok(())
    }

    fn dispute(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        let deposit = self
            .deposits
            .get_mut(&tx.tx)
            .ok_or(PaymentError::TransactionNotFound(tx.tx))?;

        if deposit.client != tx.client {
            return Err(PaymentError::TransactionNotFound(tx.tx));
        }

        if deposit.disputed {
            return Err(PaymentError::AlreadyUnderDispute(tx.tx));
        }

        deposit.disputed = true;
        let account = self
            .accounts
            .get_mut(&tx.client)
            .expect("account must exist if deposit exists");
        account.available -= deposit.amount;
        account.held += deposit.amount;

        Ok(())
    }

    fn resolve(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        let deposit = self
            .deposits
            .get_mut(&tx.tx)
            .ok_or(PaymentError::TransactionNotFound(tx.tx))?;

        if deposit.client != tx.client {
            return Err(PaymentError::TransactionNotFound(tx.tx));
        }

        if !deposit.disputed {
            return Err(PaymentError::NotUnderDispute(tx.tx));
        }

        deposit.disputed = false;
        let account = self
            .accounts
            .get_mut(&tx.client)
            .expect("account must exist if deposit exists");
        account.held -= deposit.amount;
        account.available += deposit.amount;

        Ok(())
    }

    fn chargeback(&mut self, tx: &TransactionRecord) -> Result<(), PaymentError> {
        let deposit = self
            .deposits
            .get_mut(&tx.tx)
            .ok_or(PaymentError::TransactionNotFound(tx.tx))?;

        if deposit.client != tx.client {
            return Err(PaymentError::TransactionNotFound(tx.tx));
        }

        if !deposit.disputed {
            return Err(PaymentError::NotUnderDispute(tx.tx));
        }

        deposit.disputed = false;
        let account = self
            .accounts
            .get_mut(&tx.client)
            .expect("account must exist if deposit exists");
        account.held -= deposit.amount;
        account.locked = true;

        Ok(())
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
        amount: Option<Decimal>,
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
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.total(), dec!(10));
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Deposit, 1, 2, Some(dec!(5)))).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(15));
    }

    #[test]
    fn duplicate_deposit_tx_id_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        let result = engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(99))));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
    }

    #[test]
    fn withdrawal_decreases_available() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(dec!(4)))).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(6));
        assert_eq!(account.total(), dec!(6));
    }

    #[test]
    fn withdrawal_insufficient_funds_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(5)))).unwrap();
        let result = engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(dec!(10))));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(5));
    }

    #[test]
    fn withdrawal_exact_balance() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(7)))).unwrap();
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(dec!(7)))).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
    }

    #[test]
    fn dispute_moves_to_held() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10));
        assert_eq!(account.total(), dec!(10));
    }

    #[test]
    fn dispute_nonexistent_tx_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        let result = engine.process(&tx(TransactionType::Dispute, 1, 999, None));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn dispute_already_disputed_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        let result = engine.process(&tx(TransactionType::Dispute, 1, 1, None));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.held, dec!(10));
    }

    #[test]
    fn dispute_wrong_client_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Deposit, 2, 2, Some(dec!(5)))).unwrap();
        let result = engine.process(&tx(TransactionType::Dispute, 2, 1, None));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn resolve_moves_back_to_available() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        engine.process(&tx(TransactionType::Resolve, 1, 1, None)).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(10));
    }

    #[test]
    fn resolve_not_disputed_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        let result = engine.process(&tx(TransactionType::Resolve, 1, 1, None));

        assert!(result.is_err());
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10));
    }

    #[test]
    fn chargeback_removes_held_and_locks() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        engine.process(&tx(TransactionType::Chargeback, 1, 1, None)).unwrap();

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn frozen_account_rejects_all() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        engine.process(&tx(TransactionType::Chargeback, 1, 1, None)).unwrap();

        assert!(engine.process(&tx(TransactionType::Deposit, 1, 2, Some(dec!(50)))).is_err());
        assert!(engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(dec!(1)))).is_err());

        let account = get_account(&engine, 1);
        assert!(account.locked);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
    }

    #[test]
    fn multiple_clients_independent() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        engine.process(&tx(TransactionType::Deposit, 2, 2, Some(dec!(20)))).unwrap();
        engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(dec!(5)))).unwrap();

        let a1 = get_account(&engine, 1);
        let a2 = get_account(&engine, 2);
        assert_eq!(a1.available, dec!(5));
        assert_eq!(a2.available, dec!(20));
    }

    #[test]
    fn full_dispute_resolve_lifecycle() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(100)))).unwrap();
        engine.process(&tx(TransactionType::Deposit, 1, 2, Some(dec!(50)))).unwrap();
        engine.process(&tx(TransactionType::Withdrawal, 1, 3, Some(dec!(30)))).unwrap();

        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(20));
        assert_eq!(account.held, dec!(100));
        assert_eq!(account.total(), dec!(120));

        engine.process(&tx(TransactionType::Resolve, 1, 1, None)).unwrap();
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(120));
        assert_eq!(account.held, dec!(0));
        assert!(!account.locked);
    }

    #[test]
    fn full_dispute_chargeback_lifecycle() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(100)))).unwrap();
        engine.process(&tx(TransactionType::Withdrawal, 1, 2, Some(dec!(40)))).unwrap();

        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(-40));
        assert_eq!(account.held, dec!(100));

        engine.process(&tx(TransactionType::Chargeback, 1, 1, None)).unwrap();
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(-40));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(-40));
        assert!(account.locked);
    }

    #[test]
    fn re_dispute_after_resolve() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(50)))).unwrap();
        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        engine.process(&tx(TransactionType::Resolve, 1, 1, None)).unwrap();

        engine.process(&tx(TransactionType::Dispute, 1, 1, None)).unwrap();
        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(50));
    }

    #[test]
    fn deposit_missing_amount_is_err() {
        let mut engine = PaymentEngine::new();
        let result = engine.process(&tx(TransactionType::Deposit, 1, 1, None));
        assert!(result.is_err());
    }

    #[test]
    fn withdrawal_missing_amount_is_err() {
        let mut engine = PaymentEngine::new();
        engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(10)))).unwrap();
        let result = engine.process(&tx(TransactionType::Withdrawal, 1, 2, None));
        assert!(result.is_err());
    }

    #[test]
    fn deposit_zero_amount_is_err() {
        let mut engine = PaymentEngine::new();
        let result = engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(0))));
        assert!(result.is_err());
    }

    #[test]
    fn deposit_negative_amount_is_err() {
        let mut engine = PaymentEngine::new();
        let result = engine.process(&tx(TransactionType::Deposit, 1, 1, Some(dec!(-5))));
        assert!(result.is_err());
    }
}
