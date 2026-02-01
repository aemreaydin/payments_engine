use rust_decimal::Decimal;
use rust_decimal::dec;
use serde::Serialize;

#[derive(Debug)]
pub struct Account {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn new(client: u16) -> Self {
        Self {
            client,
            available: dec!(0),
            held: dec!(0),
            locked: false,
        }
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

#[derive(Debug, Serialize)]
pub struct AccountOutput {
    pub client: u16,
    pub available: String,
    pub held: String,
    pub total: String,
    pub locked: bool,
}

impl From<&Account> for AccountOutput {
    fn from(account: &Account) -> Self {
        Self {
            client: account.client,
            available: format!("{:.4}", account.available),
            held: format!("{:.4}", account.held),
            total: format!("{:.4}", account.total()),
            locked: account.locked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_account_is_zeroed() {
        let account = Account::new(1);
        assert_eq!(account.client, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert!(!account.locked);
    }

    #[test]
    fn total_equals_available_plus_held() {
        let account = Account {
            client: 1,
            available: dec!(10),
            held: dec!(5),
            locked: false,
        };
        assert_eq!(account.total(), dec!(15));
    }

    #[test]
    fn output_formats_four_decimal_places() {
        let account = Account {
            client: 1,
            available: dec!(1.5),
            held: dec!(0),
            locked: false,
        };
        let output = AccountOutput::from(&account);
        assert_eq!(output.available, "1.5000");
        assert_eq!(output.held, "0.0000");
        assert_eq!(output.total, "1.5000");
        assert!(!output.locked);
    }

    #[test]
    fn output_formats_round_numbers() {
        let account = Account {
            client: 2,
            available: dec!(3),
            held: dec!(2),
            locked: true,
        };
        let output = AccountOutput::from(&account);
        assert_eq!(output.client, 2);
        assert_eq!(output.available, "3.0000");
        assert_eq!(output.held, "2.0000");
        assert_eq!(output.total, "5.0000");
        assert!(output.locked);
    }
}
