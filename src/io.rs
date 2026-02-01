use std::io::{Read, Write};

use crate::account::AccountOutput;
use crate::engine::PaymentEngine;
use crate::error::PaymentError;
use crate::transaction::TransactionRecord;

pub fn process_csv<R: Read>(reader: R) -> Result<PaymentEngine, PaymentError> {
    let mut engine = PaymentEngine::new();

    let mut csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(reader);

    for result in csv_reader.deserialize::<TransactionRecord>() {
        let record = result?;
        if let Err(e) = engine.process(&record) {
            eprintln!("warning: skipping transaction: {e}");
        }
    }

    Ok(engine)
}

pub fn write_accounts<W: Write>(writer: W, engine: &PaymentEngine) -> Result<(), PaymentError> {
    let mut csv_writer = csv::Writer::from_writer(writer);

    for account in engine.accounts() {
        let output = AccountOutput::from(account);
        csv_writer.serialize(&output)?;
    }

    csv_writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn process_csv_basic() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,10.0
deposit,2,2,20.0
withdrawal,1,3,5.0
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let a1 = engine.accounts().find(|a| a.client == 1).unwrap();
        let a2 = engine.accounts().find(|a| a.client == 2).unwrap();
        assert_eq!(a1.available, dec!(5));
        assert_eq!(a2.available, dec!(20));
    }

    #[test]
    fn process_csv_with_whitespace() {
        let csv_data = "\
type , client , tx , amount
deposit , 1 , 1 , 10.0
withdrawal , 1 , 2 , 5.0
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let account = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(account.available, dec!(5));
    }

    #[test]
    fn process_csv_dispute_resolve() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,50.0
dispute,1,1,
resolve,1,1,
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let account = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(account.available, dec!(50));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn process_csv_dispute_chargeback() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,50.0
dispute,1,1,
chargeback,1,1,
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let account = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn write_accounts_format() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();

        let mut output = Vec::new();
        write_accounts(&mut output, &engine).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.starts_with("client,available,held,total,locked\n"));

        let lines: Vec<&str> = output_str.trim().lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(output_str.contains("2.0000"));
    }

    #[test]
    fn process_csv_flexible_columns() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,50.0
dispute,1,1
resolve,1,1
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let account = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(account.available, dec!(50));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn process_csv_decimal_precision() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,1.2345
deposit,1,2,0.0001
withdrawal,1,3,0.2346
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        let account = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(account.available, dec!(1.0000));

        let mut output = Vec::new();
        write_accounts(&mut output, &engine).unwrap();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("1.0000"));
    }

    #[test]
    fn process_csv_empty_file() {
        let csv_data = "\
type,client,tx,amount
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();
        assert_eq!(engine.accounts().count(), 0);
    }

    #[test]
    fn spec_example() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0
";
        let engine = process_csv(csv_data.as_bytes()).unwrap();

        let a1 = engine.accounts().find(|a| a.client == 1).unwrap();
        assert_eq!(a1.available, dec!(1.5));
        assert_eq!(a1.held, dec!(0));
        assert_eq!(a1.total(), dec!(1.5));
        assert!(!a1.locked);

        let a2 = engine.accounts().find(|a| a.client == 2).unwrap();
        assert_eq!(a2.available, dec!(2));
        assert_eq!(a2.held, dec!(0));
        assert_eq!(a2.total(), dec!(2));
        assert!(!a2.locked);
    }
}
