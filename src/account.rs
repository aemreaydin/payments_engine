use serde::Serialize;

#[derive(Debug)]
pub struct Account {
    pub client: u16,
    pub available: f64,
    pub held: f64,
    pub locked: bool,
}

impl Account {
    pub fn new(client: u16) -> Self {
        Self {
            client,
            available: 0.0,
            held: 0.0,
            locked: false,
        }
    }

    pub fn total(&self) -> f64 {
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
        assert_eq!(account.available, 0.0);
        assert_eq!(account.held, 0.0);
        assert!(!account.locked);
    }

    #[test]
    fn total_equals_available_plus_held() {
        let account = Account {
            client: 1,
            available: 10.0,
            held: 5.0,
            locked: false,
        };
        assert!((account.total() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn output_formats_four_decimal_places() {
        let account = Account {
            client: 1,
            available: 1.5,
            held: 0.0,
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
            available: 3.0,
            held: 2.0,
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
