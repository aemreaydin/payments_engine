use std::io::{Read, Write};

use crate::account::AccountOutput;
use crate::engine::PaymentEngine;
use crate::error::PaymentError;
use crate::transaction::TransactionRecord;

pub fn process_csv<R: Read>(reader: R) -> Result<PaymentEngine, PaymentError> {
    let mut engine = PaymentEngine::new();

    let mut csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(reader);

    for result in csv_reader.deserialize::<TransactionRecord>() {
        let record = result?;
        engine.process(&record);
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
        assert!((a1.available - 5.0).abs() < f64::EPSILON);
        assert!((a2.available - 20.0).abs() < f64::EPSILON);
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
        assert!((account.available - 5.0).abs() < f64::EPSILON);
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
        assert!((account.available - 50.0).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
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
        assert!((account.available).abs() < f64::EPSILON);
        assert!((account.held).abs() < f64::EPSILON);
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
        assert!((a1.available - 1.5).abs() < f64::EPSILON);
        assert!((a1.held).abs() < f64::EPSILON);
        assert!((a1.total() - 1.5).abs() < f64::EPSILON);
        assert!(!a1.locked);

        let a2 = engine.accounts().find(|a| a.client == 2).unwrap();
        assert!((a2.available - 2.0).abs() < f64::EPSILON);
        assert!((a2.held).abs() < f64::EPSILON);
        assert!((a2.total() - 2.0).abs() < f64::EPSILON);
        assert!(!a2.locked);
    }
}
